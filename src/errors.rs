use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Timeout: {0}")]
    TimeoutError(#[from] tokio::time::error::Elapsed),
    #[error("IO: {0}")]
    IOError(#[from] std::io::Error),
}
impl warp::reject::Reject for Error {}
