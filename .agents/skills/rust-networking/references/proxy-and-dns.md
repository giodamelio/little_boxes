# Proxy and DNS

Connection identity, proxy, and DNS rules that the "Conditional references" table in `SKILL.md`
points to.

## Treat proxy and DNS as connection identity

Key a connection pool by every property that changes connection security or
routing:

```text
scheme + authority + proxy route + TLS identity + protocol settings
```

Do not create a client per request. Reuse one configured client for its policy
lifetime. Also do not keep an unbounded pool. Set per-host idle limits and an
idle lifetime. Let the protocol implementation detect stale connections.

HTTP/2 and HTTP/3 multiplex requests over a connection. A connection count is
not a request concurrency limit. Bound in-flight requests separately. Drain the
old pool when proxy, trust, identity, or protocol policy changes.

## Proxy

Resolve proxy policy once per request destination. Parse `NO_PROXY` with a maintained
implementation, not a home-grown suffix matcher.

Never log proxy credentials. Keep origin credentials and proxy credentials in separate
configuration. Send `Proxy-Authorization` only to the proxy. Do not reuse origin `Authorization` or
cookies as proxy authentication. For HTTPS through `CONNECT`, send origin headers and client
certificates only inside the verified end-to-end TLS tunnel. A plaintext forward proxy can inspect
origin requests, so do not send secrets through it unless the product policy accepts that trust
boundary.

## DNS

Apply DNS changes to new connections. Do not assume that an existing pooled connection follows a
changed DNS record. Bound the DNS cache by the resolver TTL or by a shorter product cap. Do not
cache a failure forever. Keep all returned addresses and use the IPv6 and IPv4 fallback strategy
of the stack. Do not select the first address permanently.

## Sources

- [RFC 8305](https://www.rfc-editor.org/rfc/rfc8305.html): Happy Eyeballs, IPv6 and IPv4 racing.
