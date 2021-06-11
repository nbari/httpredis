use serde::Serialize;
use std::convert::Infallible;
use warp::http::StatusCode;
use warp::{reject::Reject, Rejection, Reply};

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

#[derive(Debug)]
pub struct RequestTimeout;
impl Reject for RequestTimeout {}

#[derive(Debug)]
pub struct ServiceUnavailable;
impl Reject for ServiceUnavailable {}

/// # Errors
/// Infallible
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;

    if err.find::<RequestTimeout>().is_some() {
        code = StatusCode::REQUEST_TIMEOUT;
        message = "Failed to connect, connection timed out";
    } else if err.find::<ServiceUnavailable>().is_some() {
        code = StatusCode::SERVICE_UNAVAILABLE;
        message = "service unavailable";
    } else {
        eprintln!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "Internal Server Error";
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message: message.into(),
    });

    Ok(warp::reply::with_status(json, code))
}
