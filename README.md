# service-binding

Provides a way for servers and clients to describe their service
bindings and client endpoints in a structured URI format.

This crate automates parsing and binding to TCP and Unix sockets.

By design this crate has no dependencies other than what is in `std`.

## Example

```rust
use service_binding::Listener;

let host = "tcp://127.0.0.1:8012"; // or "unix:///tmp/socket"

let listener: Listener = host.parse().unwrap();

match listener {
    Listener::Unix(listener) => {
        // bind to a unix domain socket
    },
    Listener::Tcp(listener) => {
        // bind to a TCP socket
    }
}
```

## systemd Socket Activation

This crate also supports systemd's [Socket Activation][]. If the
argument to be parsed is `fd://` the `Listener` object returned will
be a `Unix` variant containing the listener provided by systemd.

[Socket Activation]: https://0pointer.de/blog/projects/socket-activation.html
