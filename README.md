# Sirpent (in Rust)

## Configuring TLS
For development purposes, generate your own self-signed certificate with:

``` sh
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
```

For production purposes we'll want to use a signed certificate **and** enforce certificate validation.
