use anyhow::{bail, Result};
use chrono::prelude::*;
use futures::{SinkExt, StreamExt};
use httpredis::{
    options,
    rejections::{handle_rejection, ServiceUnavailable},
};
use std::{
    net::{IpAddr, Ipv4Addr},
    process,
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use tokio::{net::TcpStream, sync::Mutex, time::timeout};
use tokio_native_tls::{TlsConnector, TlsStream};
use tokio_util::codec::{Framed, LinesCodec};
use warp::{http::StatusCode, Filter};

#[tokio::main]
async fn main() {
    if let Err(err) = try_main().await {
        eprintln!("ERROR: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| eprintln!("because: {}", cause));
        process::exit(1);
    }
}

async fn try_main() -> Result<()> {
    let redis: options::Redis = options::new()?;
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

    // TCP connect
    let conn = timeout(Duration::from_secs(3), TcpStream::connect(&redis.host)).await??;

    // TLS
    let stream = TlsConnector::from(redis.tls.clone())
        .connect(&redis.host, conn)
        .await?;

    let mut frame_stream = Framed::new(stream, LinesCodec::new());

    if let Some(pass) = redis.pass {
        let msg = format!("AUTH {}\r\n", pass);
        frame_stream.send(msg).await?;

        while let Some(line) = frame_stream.next().await {
            if let Ok(line) = line {
                if line != "+OK" {
                    bail!("AUTH failed");
                }
            }
            break;
        }
    }

    let client = Arc::new(Mutex::new(frame_stream));

    let now = Utc::now();
    println!(
        "{} - Listening on *:{}",
        now.to_rfc3339_opts(SecondsFormat::Secs, true),
        port
    );

    let args = warp::any().map(move || client.clone());

    let state_route = warp::any()
        .and(args)
        .and_then(state_handler)
        .recover(handle_rejection);

    warp::serve(state_route).run((addr, port)).await;
    Ok(())
}

// state_handler return HTTP 100 if role:master otherwise 200
// OK, otherwise HTTP 503 Service Unavailable
async fn state_handler(
    client: Arc<Mutex<Framed<TlsStream<TcpStream>, LinesCodec>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut stream = client.lock().await;
    let mut is_master = false;

    let msg = "info replication";
    stream
        .send(msg)
        .await
        .map_err(|e| warp::reject::custom(ServiceUnavailable(e.to_string())))?;
    while let Some(line) = stream.next().await {
        let line = line.map_err(|e| warp::reject::custom(ServiceUnavailable(e.to_string())))?;
        if line == "role:master" {
            is_master = true;
            break;
        }
    }

    return if is_master {
        // check that uptime is at least 10 seconds to prevent having multiple masters
        // an old-master when starting can start has master before going into replicaof
        let msg = "info server";
        stream
            .send(msg)
            .await
            .map_err(|e| warp::reject::custom(ServiceUnavailable(e.to_string())))?;
        while let Some(line) = stream.next().await {
            let line = line.map_err(|e| warp::reject::custom(ServiceUnavailable(e.to_string())))?;
            if line.starts_with("uptime_in_seconds:") {
                let v: Vec<&str> = line.split_terminator(':').collect();
                if v[1]
                    .trim()
                    .parse::<usize>()
                    .map_err(|e| warp::reject::custom(ServiceUnavailable(e.to_string())))?
                    > 10
                {
                    break;
                }
                return Ok(StatusCode::SERVICE_UNAVAILABLE);
            }
        }
        Ok(StatusCode::OK)
    } else {
        Ok(StatusCode::SERVICE_UNAVAILABLE)
    };
}
