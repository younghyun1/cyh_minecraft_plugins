# Minecraft timing probe

Measure TCP connection, status response and ping/pong time without logging in or changing the server. Targets must be explicitly supplied as `IP:PORT`, or `[IPv6]:PORT`. Hostname resolution is outside this diagnostic so DNS time cannot distort its connection measurements.

```sh
cargo run --package mc-lag-probe -- 127.0.0.1:25565
cargo run --package mc-lag-probe -- '[::1]:25565'
```

Each invocation performs five rounds against one to sixteen targets, with a 200 ms delay between rounds, five-second socket timeouts and a one-MiB response-frame limit. Status contents are not printed. The report prints target, connection time, status time, pong time and status packet byte length. A failed connection or invalid protocol frame stops the invocation with a nonzero exit status.

Tests use in-memory frames; they do not probe a server. No target list, server address or diagnostic result is bundled.
