// use bytes::Bytes;
use chrono::prelude::*;
use httpredis::options;
use std::net::{IpAddr, Ipv4Addr};
use std::process;
use std::str::FromStr;
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
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

    let port = redis.port;

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
        port
    );

    let args = warp::any().map(move || redis.clone());
    let state_route = warp::any().and(args).and_then(state_handler);
    warp::serve(state_route).run((addr, port)).await
}

// state_handler return HTTP 100 if role:master otherwise 200
// OK, otherwise HTTP 503 Service Unavailable
async fn state_handler(redis: options::Redis) -> Result<impl warp::Reply, warp::Rejection> {
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

    let mut buf = BufStream::new(stream);

    // AUTH
    if let Some(pass) = redis.pass {
        let msg = format!("AUTH {}", pass);
        buf.write_all(msg.as_bytes()).await.unwrap();
        buf.write_all(b"\r\n").await.unwrap();
        buf.flush().await.unwrap();

        let mut buffer = String::new();
        buf.read_line(&mut buffer).await.unwrap();
        if "+OK\r\n" != buffer {
            return Ok(StatusCode::UNAUTHORIZED);
        }
    }

    // role:master
    buf.write_all(b"info replication").await.unwrap();
    buf.write_all(b"\r\n").await.unwrap();
    buf.flush().await.unwrap();

    let mut is_master = false;
    loop {
        let mut line = String::new();
        buf.read_line(&mut line).await.unwrap();
        if line.starts_with("role:master") {
            is_master = true;
        } else if line == "\r\n" {
            break;
        }
    }
    if is_master {
        // uptime
        // to prevent having multiple masters
        // an old-master when starting can start has master before going into replicaof
        buf.write_all(b"info server").await.unwrap();
        buf.write_all(b"\r\n").await.unwrap();
        buf.flush().await.unwrap();
        loop {
            let mut line = String::new();
            buf.read_line(&mut line).await.unwrap();
            if line.starts_with("uptime_in_seconds:") {
                let v: Vec<&str> = line.split_terminator(':').collect();
                if v[1].trim().parse::<usize>().unwrap() > 10 {
                    return Ok(StatusCode::OK);
                }
            } else if line == "\r\n" {
                return Ok(StatusCode::OK);
            }
        }
    } else {
        return Ok(StatusCode::SERVICE_UNAVAILABLE);
    }
}
