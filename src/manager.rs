use body::Body;
use curl;
use error::Error;
use http;
use os_pipe;
use slab::Slab;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::os::unix::io::AsRawFd;
use std::str;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const DEFAULT_TIMEOUT_MS: u64 = 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token(usize);

pub struct ManagerHandle {
    message_sender: mpsc::Sender<Message>,
    notify_writer: os_pipe::PipeWriter,
    join_handle: thread::JoinHandle<()>,
}

impl ManagerHandle {
    /// Create a new manager.
    ///
    /// This is a fairly heavy operation.
    pub fn new() -> Result<Self, Error> {
        let (notify_reader, notify_writer) = os_pipe::pipe()?;
        let (message_sender, message_receiver) = mpsc::channel();

        let join_handle = thread::spawn(move || {
            let mut notify_fd = curl::multi::WaitFd::new();
            notify_fd.set_fd(notify_reader.as_raw_fd());
            notify_fd.poll_on_read(true);

            let mut inner = Manager {
                multi: curl::multi::Multi::new(),
                handles: Slab::new(),
                message_receiver: message_receiver,
                notify_reader: notify_reader,
                wait_fds: [notify_fd],
            };

            inner.run();
        });

        Ok(Self {
            message_sender: message_sender,
            notify_writer: notify_writer,
            join_handle: join_handle,
        })
    }

    fn send(&mut self, message: Message) -> Result<(), Error> {
        self.message_sender.send(message)?;
        self.notify_writer.write(&[0])?;
        Ok(())
    }
}

struct Manager {
    /// A curl multi handle used to execute requests.
    multi: curl::multi::Multi,

    /// Handles for active requests.
    handles: Slab<ActiveRequest>,

    message_receiver: mpsc::Receiver<Message>,
    notify_reader: os_pipe::PipeReader,
    wait_fds: [curl::multi::WaitFd; 1],
}

impl Manager {
    fn run(&mut self) {
        loop {
            // TODO: Error handling
            self.dispatch();
        }
    }

    fn dispatch(&mut self) -> Result<(), Error> {
        // Determine the blocking timeout value.
        let timeout = self.multi.get_timeout()?.unwrap_or(Duration::from_millis(DEFAULT_TIMEOUT_MS));

        // Block until activity is detected or the timeout passes.
        self.multi.wait(&mut self.wait_fds, timeout)?;

        // Consume any notify bytes received while waiting.
        if self.wait_fds[0].received_read() {
            let _ = self.notify_reader.read(&mut [0; 1]);
        }

        self.handle_pending_messages();

        // Perform any pending reads or writes. If `perform()` returns less than the number of handles, one or more of
        // them are done.
        if (self.multi.perform()? as usize) < self.handles.len() {

            let mut result = None;

            self.multi.messages(|message| {
                if let Some(Err(e)) = message.result() {
                    result = Some(e);
                }

                if let Ok(token) = message.token() {
                    // self.handles.remove(token);
                }
            });
        }

        Ok(())
    }

    fn handle_pending_messages(&mut self) {
        loop {
            match self.message_receiver.try_recv() {
                Ok(Message::Begin(_)) => unimplemented!(),
                Ok(Message::Unpause(token)) => {
                    // self.handles[token.0].unpause();
                },
                Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
    }

    fn activate_request(&mut self, request: IncomingRequest) -> Result<Token, Error> {
        // Register the easy handle with the multi handle.
        let mut active_handle = self.multi.add2(request.easy_handle)?;

        // Assign a token and insert.
        let entry = self.handles.vacant_entry();
        let token = entry.key();
        active_handle.set_token(token)?;
        entry.insert(ActiveRequest {
            easy_handle: active_handle,
        });

        Ok(Token(token))
    }

    fn remove_handle(&mut self, token: Token) -> Result<Option<curl::easy::Easy2<TransferState>>, Error> {
        let active_handle = self.handles.remove(token.0);
        let inactive_handle = self.multi.remove2(active_handle)?;

        Ok(Some(inactive_handle))
    }
}

enum Message {
    Begin(IncomingRequest),
    Unpause(Token),
}

struct IncomingRequest {
    easy_handle: curl::easy::Easy2<TransferState>,
}

impl IncomingRequest {
    fn new() -> Self {
        Self {
            easy_handle: curl::easy::Easy2::new(TransferState::new()),
        }
    }
}

struct ActiveRequest {
    easy_handle: curl::multi::Easy2Handle<TransferState>,
}

impl ActiveRequest {
    fn get_state(&self) -> &TransferState {
        self.easy_handle.get_ref()
    }
}

/// Receives callbacks from curl and incrementally constructs a response.
enum TransferHandler {
    /// Request body to be sent.
    body: Body,

    /// Builder for the response object.
    response: http::response::Builder,

    /// Temporary buffer for the response body.
    buffer: ByteBuffer,
}

impl TransferHandler {
    fn new() -> Self {
        unimplemented!();
    }
}

impl curl::easy::Handler for TransferState {
    // Gets called by curl for each line of data in the HTTP request header.
    fn header(&mut self, data: &[u8]) -> bool {
        let line = match str::from_utf8(data) {
            Ok(s) => s,
            _  => return false,
        };

        // Curl calls this function for all lines in the response not part of the response body, not just for headers.
        // We need to inspect the contents of the string in order to determine what it is and how to parse it, just as
        // if we were reading from the socket of a HTTP/1.0 or HTTP/1.1 connection ourselves.

        // Is this the status line?
        if line.starts_with("HTTP/") {
            // Parse the HTTP protocol version.
            let version = match &line[0..8] {
                "HTTP/2.0" => http::Version::HTTP_2,
                "HTTP/1.1" => http::Version::HTTP_11,
                "HTTP/1.0" => http::Version::HTTP_10,
                "HTTP/0.9" => http::Version::HTTP_09,
                _ => http::Version::default(),
            };
            self.response.version(version);

            // Parse the status code.
            let status_code = match http::StatusCode::from_str(&line[9..12]) {
                Ok(s) => s,
                _ => return false,
            };
            self.response.status(status_code);

            return true;
        }

        // Is this a header line?
        if let Some(pos) = line.find(":") {
            let (name, value) = line.split_at(pos);
            let value = value[2..].trim();
            self.response.header(name, value);

            return true;
        }

        // Is this the end of the response header?
        if line == "\r\n" {
            self.header_complete = true;
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    // Gets called by curl when attempting to send bytes of the request body.
    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        self.request_body
            .read(data)
            .map_err(|_| curl::easy::ReadError::Abort)
    }

    // Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        Ok(self.buffer.push(data))
    }
}

/// I/O stream for a single active transfer.
pub struct TransferStream {}

impl Read for TransferStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.len() == 0 {
            return Ok(0);
        }

        // self.transport.as_mut().unwrap().read(buffer)
        unimplemented!();
    }
}
