use std::io;
use std::io::Read;
use std::sync::{Arc, Mutex, Weak};
use transport::Transport;
use super::*;
use futures;
use futures::channel::oneshot;
use futures::prelude::*;
use manager;


/// An HTTP client for making requests.
///
/// The client maintains a connection pool internally and is expensive to create, so we recommend re-using your clients
/// instead of discarding and recreating them.
pub struct Client {
    options: Options,
}

impl Default for Client {
    fn default() -> Client {
        Client::new(Options::default())
    }
}

impl Client {
    /// Create a new HTTP client using the given options.
    pub fn new(options: Options) -> Self {
        Self {
            options: options,
        }
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
        futures::executor::block_on(self.send_async(request))
    }

    fn send_async(&self, request: Request) -> impl Future<Item=Response, Error=Error> {
        let easy_handle = create_curl_request(request, &self.options)?;

        let (sender, receiver) = oneshot::channel();

        self.manager.begin(easy_handle, sender)?;

        receiver.then(|result| match result {
            Ok(Ok(response)) => futures::future::ok(response),
            Ok(Err(e)) => futures::future::err(e),
            Err(canceled) => unimplemented!(),
        })
    }
}

fn create_curl_request(request: Request, options: &Options) -> Result<(), Error> {
    let mut easy = curl::easy::Easy2::new(Collector {
        data: self.data.clone(),
    };

    easy.signal(false)?;

    // Configure connection based on our options struct.
    if let Some(timeout) = options.timeout {
        easy.timeout(timeout)?;
    }
    easy.connect_timeout(options.connect_timeout)?;
    easy.tcp_nodelay(options.tcp_nodelay)?;
    if let Some(interval) = options.tcp_keepalive {
        easy.tcp_keepalive(true)?;
        easy.tcp_keepintvl(interval)?;
    }

    // Configure redirects.
    match options.redirect_policy {
        RedirectPolicy::None => {
            easy.follow_location(false)?;
        }
        RedirectPolicy::Follow => {
            easy.follow_location(true)?;
        }
        RedirectPolicy::Limit(max) => {
            easy.follow_location(true)?;
            easy.max_redirections(max)?;
        }
    }

    // Set a preferred HTTP version to negotiate.
    if let Some(version) = options.preferred_http_version {
        easy.http_version(match version {
            http::Version::HTTP_10 => curl::easy::HttpVersion::V10,
            http::Version::HTTP_11 => curl::easy::HttpVersion::V11,
            http::Version::HTTP_2 => curl::easy::HttpVersion::V2,
            _ => curl::easy::HttpVersion::Any,
        })?;
    }

    // Set a proxy to use.
    if let Some(ref proxy) = options.proxy {
        easy.proxy(&format!("{}", proxy))?;
    }

    // Set the request data according to the request given.
    easy.custom_request(request.method().as_str())?;
    easy.url(&format!("{}", request.uri()))?;

    let mut headers = curl::easy::List::new();
    for (name, value) in request.headers() {
        let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
        headers.append(&header)?;
    }
    easy.http_headers(headers)?;

    // Set the request body.
    let body = request.into_parts().1;
    if !body.is_empty() {
        easy.upload(true)?;
    }
    self.data.borrow_mut().request_body = body;

    Ok(easy)
}
