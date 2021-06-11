use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Timeout: {0}")]
    TimeoutError(#[from] tokio::time::error::Elapsed),

    #[error("IO: {0}")]
    IOError(#[from] std::io::Error),

    #[error("TlsError: {0}")]
    TlsError(#[from] tokio_native_tls::native_tls::Error),
}

impl warp::reject::Reject for Error {}
