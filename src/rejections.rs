use serde::Serialize;
use std::convert::Infallible;
use std::fmt;
use warp::http::StatusCode;
use warp::{reject::Reject, Rejection, Reply};

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

#[derive(Debug)]
pub struct RequestTimeout(pub String);
impl Reject for RequestTimeout {}
impl fmt::Display for RequestTimeout {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct ServiceUnavailable(pub String);
impl Reject for ServiceUnavailable {}
impl fmt::Display for ServiceUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// # Errors
/// Infallible
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;

    if let Some(e) = err.find::<RequestTimeout>() {
        code = StatusCode::REQUEST_TIMEOUT;
        message = format!("Failed to connect, connection timed out: {}", e);
    } else if let Some(e) = err.find::<ServiceUnavailable>() {
        code = StatusCode::SERVICE_UNAVAILABLE;
        message = format!("service unavailable: {}", e);
    } else {
        eprintln!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = String::from("Internal Server Error");
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message,
    });

    Ok(warp::reply::with_status(json, code))
}
