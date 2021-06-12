use chrono::prelude::*;
use httpredis::{
    options,
    options::Redis,
    rejections::{handle_rejection, RequestTimeout, ServiceUnavailable},
};
use std::{
    net::{IpAddr, Ipv4Addr},
    process,
    str::FromStr,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
    time::timeout,
};
use tokio_native_tls::{TlsConnector, TlsStream};
use warp::{http::StatusCode, Filter};

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

    let state_route = warp::any()
        .and(args)
        .and_then(state_handler)
        .recover(handle_rejection);

    warp::serve(state_route).run((addr, port)).await;
}

// state_handler return HTTP 100 if role:master otherwise 200
// OK, otherwise HTTP 503 Service Unavailable
async fn state_handler(redis: options::Redis) -> Result<impl warp::Reply, warp::Rejection> {
    let conn = timeout(Duration::from_secs(3), TcpStream::connect(&redis.host))
        .await
        .map_err(|e| warp::reject::custom(RequestTimeout(e.to_string())))?
        .map_err(|e| warp::reject::custom(ServiceUnavailable(e.to_string())))?;

    let stream = TlsConnector::from(redis.tls.clone())
        .connect(&redis.host, conn)
        .await
        .map_err(|e| warp::reject::custom(ServiceUnavailable(e.to_string())))?;

    let mut buf = BufStream::new(stream);

    let result = check_master_status(redis, &mut buf)
        .await
        .map_err(|e| warp::reject::custom(ServiceUnavailable(e.to_string())))?;
    Ok(result)
}

async fn check_master_status(
    redis: Redis,
    buf: &mut BufStream<TlsStream<TcpStream>>,
) -> Result<StatusCode, std::io::Error> {
    // AUTH
    if let Some(pass) = redis.pass {
        let msg = format!("AUTH {}", pass);
        buf.write_all(msg.as_bytes()).await?;
        buf.write_all(b"\r\n").await?;
        buf.flush().await?;

        let mut buffer = String::new();
        buf.read_line(&mut buffer).await?;
        if "+OK\r\n" != buffer {
            return Ok(StatusCode::UNAUTHORIZED);
        }
    }

    // role:master
    buf.write_all(b"info replication\r\n").await?;
    buf.flush().await?;

    let mut is_master = false;
    let mut line = String::new();
    loop {
        line.clear();
        buf.read_line(&mut line).await?;
        if line.starts_with("role:master") {
            is_master = true;
        } else if line == "\r\n" {
            break;
        }
    }
    return if is_master {
        // uptime
        // to prevent having multiple masters
        // an old-master when starting can start has master before going into replicaof
        buf.write_all(b"info server\r\n").await?;
        buf.flush().await?;
        let mut line = String::new();
        loop {
            line.clear();
            buf.read_line(&mut line).await?;
            if line.starts_with("uptime_in_seconds:") {
                let v: Vec<&str> = line.split_terminator(':').collect();
                if v[1]
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Cannot parse!"))?
                    > 10
                {
                    break;
                }
            } else if line == "\r\n" {
                break;
            }
        }
        Ok(StatusCode::OK)
    } else {
        Ok(StatusCode::SERVICE_UNAVAILABLE)
    };
}
