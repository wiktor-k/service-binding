# service-binding

[![CI](https://github.com/wiktor-k/service-binding/actions/workflows/ci.yml/badge.svg)](https://github.com/wiktor-k/service-binding/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/service-binding)](https://crates.io/crates/service-binding)
[![Codecov](https://img.shields.io/codecov/c/gh/wiktor-k/service-binding)](https://app.codecov.io/gh/wiktor-k/service-binding)

Provides a way for servers and clients to describe their service bindings and client endpoints in a structured format.

This crate automates parsing and binding to TCP sockets, Unix sockets and [Windows Named Pipes][WNP].

[WNP]: https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipes

By design this crate has no dependencies other than what is in `std`.

## Supported schemes

Currently the crate supports parsing strings of the following formats:

- `tcp://ip:port` (e.g. `tcp://127.0.0.1:8080`) - TCP sockets,
- `unix://path` (e.g. `unix:///run/user/1000/test.sock`) - Unix domain sockets,
- `fd://` - systemd Socket Activation protocol (returns a Unix domain socket),
- `\\path` (e.g. `\\.\pipe\test`) for Windows Named Pipes.

## Examples

### Simple parsing

```rust
use service_binding::{Binding, Listener};

let host = "tcp://127.0.0.1:8080"; // or "unix:///tmp/socket"

let binding: Binding = host.parse().unwrap();

match binding.try_into().unwrap() {
    #[cfg(unix)]
    Listener::Unix(listener) => {
        // bind to a unix domain socket
    },
    Listener::Tcp(listener) => {
        // bind to a TCP socket
    }
    Listener::NamedPipe(pipe) => {
        // bind to a Windows Named Pipe
    }
}
```

### Web server

The following example uses `clap` and `actix-web` and makes it
possible to run the server using any combination of Unix domain
sockets (including systemd socket activation) and regular TCP socket
bound to a TCP port:

```rust,no_run
use actix_web::{web, App, HttpServer, Responder};
use clap::Parser;
use service_binding::{Binding, Listener};

#[derive(Parser, Debug)]
struct Args {
    #[clap(
        env = "HOST",
        short = 'H',
        long,
        default_value = "tcp://127.0.0.1:8080"
    )]
    host: Binding,
}

async fn greet() -> impl Responder {
    "Hello!"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let server = HttpServer::new(move || {
        App::new().route("/", web::get().to(greet))
    });

    match Args::parse().host.try_into()? {
        #[cfg(unix)]
        Listener::Unix(listener) => server.listen_uds(listener),
        Listener::Tcp(listener) => server.listen(listener),
        _ => Err(std::io::Error::other("Unsupported listener type")),
    }?.run().await
}
```

## systemd Socket Activation

This crate also supports systemd's [Socket Activation][]. If the
argument to be parsed is `fd://` the `Listener` object returned will
be a `Unix` variant containing the listener provided by systemd.

[Socket Activation]: https://0pointer.de/blog/projects/socket-activation.html

For example the following file defines a socket unit:
`~/.config/systemd/user/app.socket`:

```ini
[Socket]
ListenStream=%t/app.sock

[Install]
WantedBy=sockets.target
```

When enabled it will create a new socket file in `$XDG_RUNTIME_DIR`
directory. When this socket is connected to systemd will start the
service; `fd://` reads the correct systemd environment variable and
returns the Unix domain socket.

The service unit file `~/.config/systemd/user/app.service`:

```ini
[Service]
ExecStart=/usr/bin/app -H fd://
```

## License

This project is licensed under either of:

  - [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0),
  - [MIT license](https://opensource.org/licenses/MIT).

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this crate by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
