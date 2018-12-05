use failure::{Backtrace, Context, Fail};
use lapin_futures;
use std::fmt;
use std::io;

/// The type of error that can be returned in this crate.
///
/// Instead of implementing the `Error` trait provided by the standard library,
/// it implemented the `Fail` trait provided by the `failure` crate. Doing so
/// means that this type guaranteed to be both sendable and usable across
/// threads, and that you'll be able to use the downcasting feature of the
/// `failure::Error` type.
#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

/// The different kinds of errors that can be reported.
///
/// Even though we expose the complete enumeration of possible error variants, it is not
/// considered stable to exhaustively match on this enumeration: do it at your own risk.
#[derive(Debug, Fail)]
pub enum ErrorKind {
    /// Failure to parse an Uri
    #[fail(display = "Uri parsing error: {:?}", _0)]
    UriParsingError(String),
    /// Failure to resolve a domain name
    #[fail(display = "Couldn't resolve domain name: {}", _0)]
    InvalidDomainName(String),
    /// Failure to connect
    #[fail(display = "Failed to connect: {}", _0)]
    ConnectionFailed(#[fail(cause)] io::Error),
    /// Error from lapin_futures
    #[fail(display = "Protocol error: {:?}", _0)]
    ProtocolError(#[fail(cause)] lapin_futures::error::Error),
    /// A hack to prevent developers from exhaustively match on the enum's variants
    ///
    /// The purpose of this variant is to let the `ErrorKind` enumeration grow more variants
    /// without it being a breaking change for users. It is planned for the language to provide
    /// this functionnality out of the box, though it has not been [stabilized] yet.
    ///
    /// [stabilized]: https://github.com/rust-lang/rust/issues/44109
    #[doc(hidden)]
    #[fail(display = "lapin_futures_tls_internal::error::ErrorKind::__Nonexhaustive: this should not be printed")]
    __Nonexhaustive,
}

impl Error {
    /// Return the underlying `ErrorKind`
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error { inner: Context::new(kind) }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner: inner }
    }
}

impl From<lapin_futures::error::Error> for Error {
    fn from(error: lapin_futures::error::Error) -> Error {
        ErrorKind::ProtocolError(error).into()
    }
}
