use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("connection timeout: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),

    #[error("could not connect: {0}")]
    IO(#[from] std::io::Error),

    #[error("TlsError: {0}")]
    Tls(#[from] tokio_native_tls::native_tls::Error),
}

impl warp::reject::Reject for Error {}
