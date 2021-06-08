use bytes::Bytes;
use chrono::prelude::*;
use httpredis::options;
use std::net::{IpAddr, Ipv4Addr};
use std::process;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufStream},
    net::TcpStream,
    sync::{mpsc, Mutex},
    time::timeout,
};
use tokio_native_tls::TlsConnector;
use warp::http::StatusCode;
use warp::Filter;

#[tokio::main]
async fn main() {
    let redis: options::Redis = match options::new() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    let stream = match timeout(Duration::from_secs(3), TcpStream::connect(&redis.host)).await {
        Ok(conn) => match conn {
            Ok(conn) => match TlsConnector::from(redis.tls)
                .connect(&redis.host, conn)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{}", e);
                    process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("{}", e);
                process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Timeout :{}", e);
            process::exit(1);
        }
    };

    let a = Arc::new(Mutex::new(stream));
    let s = warp::any().map(move || a.clone());
    let state_route = warp::any().and(s).and_then(state_handler);

    let addr = if redis.v46 {
        // tcp46 or fallback to tcp4
        match IpAddr::from_str("::0") {
            Ok(a) => a,
            Err(_) => IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        }
    } else {
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
    };

    let now = Utc::now();
    println!(
        "{} - Listening on *:{}",
        now.to_rfc3339_opts(SecondsFormat::Secs, true),
        redis.port
    );
    warp::serve(state_route).run((addr, redis.port)).await
}

// state_handler return HTTP 100 if role:master otherwise 200
// OK, otherwise HTTP 503 Service Unavailable
async fn state_handler(
    stream: Arc<tokio::sync::Mutex<tokio_native_tls::TlsStream<tokio::net::TcpStream>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let rs = 0;
    match rs {
        4 => Ok(StatusCode::OK),
        _ => Ok(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/*
let msg = format!("AUTH {}", redis.pass.unwrap());
let mut buf = BufStream::new(stream);
buf.write_all(msg.as_bytes()).await.unwrap();
buf.write_all(b"\r\n").await.unwrap();
buf.flush().await.unwrap();

let mut buffer = String::new();
let num_bytes = buf.read_line(&mut buffer).await.unwrap();
println!("{}, {:?}", num_bytes, buffer);
assert_eq!("+OK\r\n", buffer);

buf.write_all(b"info replication").await.unwrap();
buf.write_all(b"\r\n").await.unwrap();
buf.flush().await.unwrap();

loop {
let mut line = String::new();
buf.read_line(&mut line).await.unwrap();
if line.starts_with("role:master") {
println!("{}", line);
} else if line == "\r\n" {
break;
}
}
*/
