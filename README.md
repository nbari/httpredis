# httpredis

HTTP status codes for Redis Sentinel when using TLS.

## Why ?
To offer high availability when using HAProxy and Redis + Sentinel are configured to use TLS.

## The problem to solve
When not using TLS, HAproxy can be configured like this to find the current master:

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

This works very good, but if the Redis cluster is configured to use TLS, the
`option tcp-check` doesn't work since it doesn't support TLS.


## How it works

`httpredis` is a web service that connects to Redis using TLS, when requesting
`info replication` if role is `role:master` the service will return the HTTP
status code `100`, otherwise if node is health `200`

    HTTP 100 Continue: role:master
    HTPP 200 OK:       node is healthy PING/PONG

The `HAProxy` configuration can be:

    backend redis
        mode tcp
        option httpchk
        http-check expect status 100
        default-server check port 36379
