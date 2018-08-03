use std::io;
use std::io::Read;
use std::sync::{Arc, Mutex, Weak};
use transport::Transport;
use super::*;

const PRELOADED_TRANSPORTS: usize = 32;


/// An HTTP client for making requests.
///
/// The client maintains a connection pool internally and is expensive to create, so we recommend re-using your clients
/// instead of discarding and recreating them.
pub struct Client {
    //max_connections: Option<u16>,
    options: Options,
    transport_pool: Arc<Mutex<Vec<Transport>>>,
    //transport_count: u16,
}

impl Default for Client {
    fn default() -> Self {
        let options: Options = Default::default();
        let mut transport_pool = Vec::with_capacity(PRELOADED_TRANSPORTS);
        for _ in 0..PRELOADED_TRANSPORTS {
            transport_pool.push(Transport::with_options(options.clone()));
        }
        let transport_pool = Arc::new(Mutex::new(transport_pool));
        Self { options, transport_pool }
    }
}

impl Client {
    pub fn with_options(options: Options) -> Self {
        let mut transport_pool = Vec::with_capacity(PRELOADED_TRANSPORTS);
        for _ in 0..PRELOADED_TRANSPORTS {
            transport_pool.push(Transport::with_options(options.clone()));
        }
        let transport_pool = Arc::new(Mutex::new(transport_pool));
        Self { options, transport_pool }
    }

    /// Sends a GET request.
    pub fn get(&self, uri: &str) -> Result<Response, Error> {
        let request = http::Request::get(uri).body(Body::Empty)?;
        self.send(request)
    }

    /// Sends a POST request.
    pub fn post<B: Into<Body>>(&self, uri: &str, body: B) -> Result<Response, Error> {
        let request = http::Request::post(uri).body(body.into())?;
        self.send(request)
    }

    /// Sends a PUT request.
    pub fn put<B: Into<Body>>(&self, uri: &str, body: B) -> Result<Response, Error> {
        let request = http::Request::put(uri).body(body.into())?;
        self.send(request)
    }

    /// Sends a DELETE request.
    pub fn delete(&self, uri: &str) -> Result<Response, Error> {
        let request = http::Request::delete(uri).body(Body::Empty)?;
        self.send(request)
    }

    /// Sends a request and returns the response.
    pub fn send(&self, request: Request) -> Result<Response, Error> {
        if let Some(mut transport) = self.get_transport() {
            let mut response = transport.execute(request)?;
            let stream = self.create_stream(transport);

            response
                .body(Body::from_reader(stream))
                .map_err(Into::into)
        } else {
            Err(Error::TooManyConnections)
        }
    }

    /// Note - this can no longer fail, as `max_connections` check disabled
    ///
    fn get_transport(&self) -> Option<Transport> {
        let mut pool = self.transport_pool.lock().unwrap();

        if let Some(transport) = pool.pop() {
            return Some(transport);
        }

        // if let Some(max) = self.max_connections {
        //     if self.transport_count >= max {
        //         return None;
        //     }
        // }

        Some(self.create_transport())
    }

    fn create_transport(&self) -> Transport {
        Transport::with_options(self.options.clone())
    }

    fn create_stream(&self, transport: Transport) -> Stream {
        Stream {
            pool: Arc::downgrade(&self.transport_pool),
            transport: Some(transport),
        }
    }
}

/// Stream that reads the response body incrementally.
///
/// A stream object will hold on to the connection that initiated the request until the entire response is read or the
/// stream is dropped.
struct Stream {
    pool: Weak<Mutex<Vec<Transport>>>,
    transport: Option<Transport>,
}

impl Read for Stream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.transport.as_mut().unwrap().read(buffer)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if let Some(transport) = self.transport.take() {
            if let Some(pool) = self.pool.upgrade() {
                pool.lock()
                    .unwrap()
                    .push(transport);
            }
        }
    }
}
