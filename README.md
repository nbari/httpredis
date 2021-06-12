# httpredis

[![build](https://github.com/nbari/httpredis/actions/workflows/rust.yml/badge.svg)](https://github.com/nbari/httpredis/actions/workflows/rust.yml)

HTTP status codes for Redis Sentinel when using TLS.

## Why ?
To offer high availability when using HAProxy and Redis + Sentinel are configured to use TLS.

## The problem to solve
When not using TLS, HAProxy can be configured like this to find the current master:

    backend redis
        mode tcp
        balance first
        timeout queue 5s
        default-server check inter 1s fall 2 rise 2 maxconn 500
        option tcp-check
        tcp-check connect
        tcp-check send AUTH\ secret\r\n
        tcp-check expect string +OK
        tcp-check send PING\r\n
        tcp-check expect string +PONG
        tcp-check send info\ replication\r\n
        tcp-check expect string role:master
        tcp-check send info\ server\r\n
        tcp-check expect rstring uptime_in_seconds:\d{2,}
        tcp-check send QUIT\r\n
        tcp-check expect string +OK
        server redis1 10.0.1.11:6379
        server redis2 10.0.1.12:6379
        server redis3 10.0.1.13:6379

This works very good, but if Redis is configured to use TLS, the
`option tcp-check` doesn't work since it doesn't support TLS.


## How it works

`httpredis` is a web service that connects to Redis using TLS, when requesting
`info replication` if role is `role:master` the service will return the HTTP
status code `200`.

The `HAProxy` configuration can be:

    backend redis
        mode tcp
        option httpchk
        default-server check inter 1s fall 2 rise 2 port 36379
        server redis1 10.0.1.11:6379 ssl crt /path/to/redis.pem ca-file /path/to/ca.crt ssl verify none
        server redis2 10.0.1.11:6379 ssl crt /path/to/redis.pem ca-file /path/to/ca.crt ssl verify none
        server redis3 10.0.1.11:6379 ssl crt /path/to/redis.pem ca-file /path/to/ca.crt ssl verify none

`redis.pem` is the `redis.crt` + `redis.key`:

    $ cat redis.crt redis.key > redis.pem

Every Redis node need to run `httpredis`:

    $ cargo install httpredis

Then run it like this:

    httpredis --tls-ca-cert-file /path/to/ca.crt --tls-cert-file /path/to/redis.crt --tls-key-file /path/to/redis.key --pass secret

> check `httpredis -h` for more options
