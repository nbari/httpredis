use anyhow::{Context, Result};
use clap::{App, Arg};
use native_tls::{Certificate, Identity, TlsConnector};
use openssl::pkcs12::Pkcs12;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use rand::{distributions::Alphanumeric, Rng};
use std::fs;
use std::io::Read;

#[derive(Clone, Debug)]
pub struct Redis {
    pub host: String,
    pub user: Option<String>,
    pub pass: Option<String>,
    pub v46: bool,
    pub port: u16,
    pub tls: native_tls::TlsConnector,
}

fn load_ca(filename: &str) -> Result<native_tls::Certificate> {
    let mut buf = Vec::new();
    fs::File::open(filename)?.read_to_end(&mut buf)?;
    Ok(Certificate::from_pem(&buf)?)
}

fn load_cert(filename: &str) -> Result<X509> {
    let mut buf = Vec::new();
    fs::File::open(filename)?.read_to_end(&mut buf)?;
    Ok(X509::from_pem(&buf)?)
}

fn load_key(filename: &str) -> Result<PKey<Private>> {
    let mut buf = Vec::new();
    fs::File::open(filename)?.read_to_end(&mut buf)?;
    Ok(PKey::private_key_from_pem(&buf)?)
}

fn is_file(s: String) -> Result<(), String> {
    if fs::metadata(&s).map_err(|e| e.to_string())?.is_file() {
        Ok(())
    } else {
        Err(format!(
            "cannot read the file: {}, verify file exist and is not a directory.",
            s
        ))
    }
}

fn random_string() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn is_num(s: String) -> Result<(), String> {
    if let Err(..) = s.parse::<usize>() {
        return Err(String::from("Not a valid number!"));
    }
    Ok(())
}

// returns (v46, port, pool)
pub fn new() -> Result<Redis> {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("redis")
                .takes_value(true)
                .default_value("127.0.0.1:36379")
                .help("redis host:port")
                .long("host")
                .required(true),
        )
        .arg(
            Arg::with_name("user")
                .takes_value(true)
                .help("redis user")
                .long("user")
                .short("u"),
        )
        .arg(
            Arg::with_name("pass")
                .takes_value(true)
                .help("redis password")
                .long("pass")
                .short("p"),
        )
        .arg(
            Arg::with_name("ca")
                .takes_value(true)
                .help("/path/to/ca.crt")
                .long("tls-ca-cert-file")
                .validator(is_file),
        )
        .arg(
            Arg::with_name("crt")
                .takes_value(true)
                .help("/path/to/redis.crt")
                .long("tls-cert-file")
                .validator(is_file)
                .required(true),
        )
        .arg(
            Arg::with_name("key")
                .takes_value(true)
                .help("/path/to/redis.key")
                .long("tls-key-file")
                .validator(is_file)
                .required(true),
        )
        .arg(
            Arg::with_name("port")
                .default_value("36379")
                .help("listening HTTP port")
                .long("http-port")
                .validator(is_num)
                .required(true),
        )
        .arg(
            Arg::with_name("v46")
                .help("listen in both IPv4 and IPv6")
                .long("46"),
        )
        .get_matches();

    let mut tls_builder = TlsConnector::builder();

    if matches.is_present("ca") {
        let pem_file = matches.value_of("ca").unwrap();
        let ca_cert = load_ca(pem_file)?;
        tls_builder.add_root_certificate(ca_cert);
    }

    if matches.is_present("crt") && matches.is_present("key") {
        let crt_file = matches.value_of("crt").unwrap();
        let client_cert: X509 = load_cert(crt_file)?;

        let key_file = matches.value_of("key").unwrap();
        let client_key = load_key(key_file)?;

        let builder = Pkcs12::builder();
        let pass = random_string();
        let pkcs12 = builder.build(&pass, "httpredis", &client_key, &client_cert)?;
        let pkcs12_der = pkcs12.to_der()?;
        let identity = Identity::from_pkcs12(&pkcs12_der, &pass)?;
        tls_builder.identity(identity);
    }

    tls_builder.danger_accept_invalid_certs(true);
    let tls = tls_builder.build()?;

    let host = matches
        .value_of("redis")
        .context("missing redis host:port")?;

    let host_port = host.split(':');
    let host = if host_port.count() == 2 {
        host.to_string()
    } else {
        format!("{}:6379", host)
    };

    let user = matches.value_of("user").map(|p| p.to_string());

    let pass = matches.value_of("pass").map(|p| p.to_string());

    Ok(Redis {
        host,
        user,
        pass,
        v46: matches.is_present("v46"),
        port: matches.value_of("port").unwrap().parse::<u16>()?,
        tls,
    })
}
