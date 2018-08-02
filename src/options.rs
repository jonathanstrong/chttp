use http::{self, Uri};
use std::time::Duration;


/// Defines various protocol and connection options.
#[derive(Clone, Debug)]
pub struct Options {
    /// The policy for automatically following server redirects.
    ///
    /// The default is to not follow redirects.
    pub redirect_policy: RedirectPolicy,

    /// A preferred HTTP version the client should attempt to use to communicate to the server with.
    ///
    /// This is treated as a suggestion. A different version may be used if the server does not support it or negotiates
    /// a different version.
    ///
    /// The default value is `None` (any version).
    pub preferred_http_version: Option<http::Version>,

    /// A timeout for the maximum time allowed for a request-response cycle.
    ///
    /// The default value is `None` (unlimited).
    pub timeout: Option<Duration>,

    /// A timeout for the initial connection phase.
    ///
    /// The default value is 300 seconds.
    pub connect_timeout: Duration,

    /// Enable or disable TCP keepalive with a given probe interval.
    ///
    /// The default value is `None` (disabled).
    pub tcp_keepalive: Option<Duration>,

    /// Enable or disable the `TCP_NODELAY` option.
    ///
    /// The default value is `false`.
    pub tcp_nodelay: bool,

    /// Indicates whether the `Referer` header should be automatically updated.
    pub auto_referer: bool,

    /// A proxy to use for requests.
    ///
    /// The proxy protocol is specified by the URI scheme.
    ///
    /// - **`http`**: Proxy. Default when no scheme is specified.
    /// - **`https`**: HTTPS Proxy. (Added in 7.52.0 for OpenSSL, GnuTLS and NSS)
    /// - **`socks4`**: SOCKS4 Proxy.
    /// - **`socks4a`**: SOCKS4a Proxy. Proxy resolves URL hostname.
    /// - **`socks5`**: SOCKS5 Proxy.
    /// - **`socks5h`**: SOCKS5 Proxy. Proxy resolves URL hostname.
    pub proxy: Option<Uri>,

    /// Specify ciphers to use for TLS.
    ///
    /// Holds the list of ciphers to use for the SSL connection. The list must be syntactically correct, it consists of one or more cipher strings separated by colons. Commas or spaces are also acceptable separators but colons are normally used, !, - and + can be used as operators.
    ///
    /// For OpenSSL and GnuTLS valid examples of cipher lists include 'RC4-SHA', ´SHA1+DES´, 'TLSv1' and 'DEFAULT'. The default list is normally set when you compile OpenSSL.
    ///
    /// You'll find more details about cipher lists on this URL:
    ///
    /// https://www.openssl.org/docs/apps/ciphers.html
    ///
    /// For NSS, valid examples of cipher lists include 'rsa_rc4_128_md5', ´rsa_aes_128_sha´, etc. With NSS you don't add/remove ciphers. If one uses this option then all known ciphers are disabled and only those passed in are enabled.
    ///
    /// You'll find more details about the NSS cipher lists on this URL:
    ///
    /// http://git.fedorahosted.org/cgit/mod_nss.git/plain/docs/mod_nss.html#Directives
    ///
    /// By default this option is not set and corresponds to CURLOPT_SSL_CIPHER_LIST.
    ///
    pub ssl_cipher_list: Option<String>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            redirect_policy: RedirectPolicy::default(),
            preferred_http_version: None,
            timeout: None,
            connect_timeout: Duration::from_secs(300),
            tcp_keepalive: None,
            tcp_nodelay: false,
            auto_referer: false,
            proxy: None,
            ssl_cipher_list: None,
        }
    }
}

impl Options {
    pub fn new() -> Options {
        Options::default()
    }
}


/// Describes a policy for handling server redirects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectPolicy {
    /// Do not apply any special treatment to redirect responses. The response will be return as-is and redirects will
    /// not be followed.
    ///
    /// This is the default policy.
    None,
    /// Follow all redirects automatically.
    Follow,
    /// Follow redirects automatically up to a maximum number of redirects.
    Limit(u32),
}

impl Default for RedirectPolicy {
    fn default() -> RedirectPolicy {
        RedirectPolicy::None
    }
}
