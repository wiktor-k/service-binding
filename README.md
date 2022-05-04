# service-binding

Provides a way for servers and clients to describe their service
bindings and client endpoints in a structured URI format.

This crate automates parsing and binding to TCP and Unix sockets.

By design this crate has no dependencies other than what is in `std`.

## Examples

### Simple parsing

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

### Web server

The following example uses `clap` and `actix-web` and makes it
possible to run the server using any combination of Unix domain
sockets (including systemd activation) and regular TCP socket bound to
a TCP port:

```rust,no_run
use actix_web::{web, App, HttpServer, Responder};
use clap::Parser;
use service_binding::Listener;

#[derive(Parser, Debug)]
struct Args {
    #[clap(env = "HOST", short = 'H', long, default_value = "tcp://127.0.0.1:8080")]
    host: String,
}

async fn greet() -> impl Responder {
    "Hello!"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let server = HttpServer::new(move || {
        App::new().route("/", web::get().to(greet))
    });

    match args.host.parse()? {
        Listener::Unix(listener) => server.listen_uds(listener)?,
        Listener::Tcp(listener) => server.listen(listener)?,
    }.run().await
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
