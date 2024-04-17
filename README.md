# service-binding

[![CI](https://github.com/wiktor-k/service-binding/actions/workflows/rust.yml/badge.svg)](https://github.com/wiktor-k/service-binding/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/service-binding)](https://crates.io/crates/service-binding)

Provides a way for servers and clients to describe their service bindings and client endpoints in a structured format.

This crate automates parsing and binding to TCP sockets, Unix sockets and [Windows Named Pipes][WNP].

[WNP]: https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipes

By design this crate is very lean and mostly relies on what is in `std` (with an exception of macOS launchd service binding).

The URI scheme bindings have been heavily inspired by how [Docker Engine] specifies them.

[Docker Engine]: https://docs.docker.com/desktop/faqs/general/#how-do-i-connect-to-the-remote-docker-engine-api

## Supported schemes

Currently the crate supports parsing strings of the following formats:

- `tcp://ip:port` (e.g. `tcp://127.0.0.1:8080`) - TCP sockets,
- `unix://path` (e.g. `unix:///run/user/1000/test.sock`) - Unix domain sockets, not available on Windows through the `std` right now (see [#271] and [#56533]),
- `fd://` - socket activation protocol (returns a Unix domain socket):
  - `fd://` - take the single socket from systemd (equivalent of `fd://3` but fails if more sockets have been passed) *listener only*,
  - `fd://<number>` - use an exact number as a file descriptor,
  - `fd://<socket-name>` - use socket activation by name, *listener only*,
- `npipe://<path>` (e.g. `npipe://test`) for Windows Named Pipes (translates to `\\.\pipe\test`).

[#271]: https://github.com/rust-lang/libs-team/issues/271
[#56533]: https://github.com/rust-lang/rust/issues/56533

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
FileDescriptorName=service-name

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

Since the socket is named (`FileDescriptorName=service-name`) it can
also be selected using its explicit name: `fd://service-name`.

## launchd Socket Activation

On macOS [launchd socket activation][LSA] is also available although the socket
needs to be explicitly named through the `fd://socket-name` syntax.

[LSA]: https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html

The corresponding `plist` file (which can be placed in `~/Library/LaunchAgents`
and loaded via `launchctl load ~/Library/LaunchAgents/service.plist`):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>EnvironmentVariables</key>
	<dict>
		<key>RUST_LOG</key>
		<string>debug</string>
	</dict>
	<key>KeepAlive</key>
	<true/>
	<key>Label</key>
	<string>com.example.service</string>
	<key>OnDemand</key>
	<true/>
	<key>ProgramArguments</key>
	<array>
		<string>/path/to/service</string>
		<string>-H</string>
		<string>fd://socket-name</string> <!-- activate socket by name -->
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>Sockets</key>
	<dict>
		<key>socket-name</key> <!-- the socket name here -->
		<dict>
			<key>SockPathName</key>
			<string>/path/to/socket</string>
			<key>SockFamily</key>
			<string>Unix</string>
		</dict>
	</dict>
	<key>StandardErrorPath</key>
	<string>/Users/test/Library/Logs/service/stderr.log</string>
	<key>StandardOutPath</key>
	<string>/Users/test/Library/Logs/service/stdout.log</string>
</dict>
</plist>
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
