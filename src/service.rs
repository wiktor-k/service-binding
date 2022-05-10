use super::Error;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

/// Service binding.
///
/// Indicates which mechanism should the service take to bind its
/// listener to.
///
/// # Examples
///
/// ```
/// # use service_binding::Binding;
/// let binding = "tcp://127.0.0.1:8080".try_into().unwrap();
/// assert_eq!(Binding::Socket(([127, 0, 0, 1], 8080).into()), binding);
/// ```
#[derive(Debug, PartialEq)]
pub enum Binding {
    /// The service should be bound to this explicit, opened file
    /// descriptor.  This mechanism is used by systemd socket
    /// activation.
    FileDescriptor(i32),

    /// The service should be bound to a Unix domain socket file under
    /// specified path.
    FilePath(PathBuf),

    /// The service should be bound to a TCP socket with given
    /// parameters.
    Socket(SocketAddr),
}

/// Opened service listener.
///
/// This structure contains an already open listener. Note that the
/// listeners are set to non-blocking mode.
///
/// # Examples
///
/// ```
/// # use service_binding::{Binding, Listener};
/// let binding: Binding = "unix:///tmp/socket".parse().unwrap();
/// let listener = binding.try_into().unwrap();
/// assert!(matches!(listener, Listener::Unix(_)));
/// ```
#[derive(Debug)]
pub enum Listener {
    /// Listener for a Unix domain socket.
    Unix(UnixListener),

    /// Listener for a TCP socket.
    Tcp(TcpListener),
}

impl From<UnixListener> for Listener {
    fn from(listener: UnixListener) -> Self {
        while let Err(e) = listener.set_nonblocking(true) {
            // retry WouldBlock errors
            if e.kind() != std::io::ErrorKind::WouldBlock {
                break;
            }
        }

        Listener::Unix(listener)
    }
}

impl From<TcpListener> for Listener {
    fn from(listener: TcpListener) -> Self {
        while let Err(e) = listener.set_nonblocking(true) {
            // retry WouldBlock errors
            if e.kind() != std::io::ErrorKind::WouldBlock {
                break;
            }
        }

        Listener::Tcp(listener)
    }
}

impl<'a> std::convert::TryFrom<&'a str> for Binding {
    type Error = Error;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        if s == "fd://" {
            if let Ok(fds) = std::env::var("LISTEN_FDS") {
                let fds: i32 = fds.parse().map_err(|_| Error)?;

                // we support only one socket for now
                if fds != 1 {
                    return Err(Error);
                }

                Ok(Binding::FileDescriptor(fds + 2))
            } else {
                Err(Error)
            }
        } else if let Some(file) = s.strip_prefix("unix://") {
            Ok(Binding::FilePath(file.into()))
        } else if let Some(addr) = s.strip_prefix("tcp://") {
            let addr: SocketAddr = addr.parse()?;
            Ok(Binding::Socket(addr))
        } else {
            Err(Error)
        }
    }
}

impl std::str::FromStr for Binding {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.try_into()
    }
}

impl TryFrom<Binding> for Listener {
    type Error = std::io::Error;

    fn try_from(value: Binding) -> Result<Self, Self::Error> {
        match value {
            Binding::FileDescriptor(descriptor) => {
                use std::os::unix::io::FromRawFd;

                Ok(unsafe { UnixListener::from_raw_fd(descriptor) }.into())
            }
            Binding::FilePath(path) => {
                // ignore errors if the file does not exist
                let _ = std::fs::remove_file(&path);
                Ok(UnixListener::bind(path)?.into())
            }
            Binding::Socket(socket) => Ok(std::net::TcpListener::bind(&socket)?.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fd() -> Result<(), Error> {
        std::env::set_var("LISTEN_FDS", "1");
        let binding = "fd://".try_into()?;
        assert_eq!(Binding::FileDescriptor(3), binding);
        Ok(())
    }

    #[test]
    fn parse_fd_fail() -> Result<(), Error> {
        std::env::remove_var("LISTEN_FDS");
        assert!(matches!(
            "fd://".try_into() as Result<Binding, _>,
            Err(Error)
        ));
        Ok(())
    }

    #[test]
    fn parse_unix() -> Result<(), Error> {
        let binding = "unix:///tmp/test".try_into()?;
        assert_eq!(Binding::FilePath("/tmp/test".into()), binding);
        Ok(())
    }

    #[test]
    fn parse_tcp() -> Result<(), Error> {
        let binding = "tcp://127.0.0.1:8080".try_into()?;
        assert_eq!(Binding::Socket(([127, 0, 0, 1], 8080).into()), binding);
        Ok(())
    }

    #[test]
    fn parse_unknown_fail() -> Result<(), Error> {
        assert!(matches!(
            "unknown://test".try_into() as Result<Binding, _>,
            Err(Error)
        ));
        Ok(())
    }

    #[test]
    fn listen_on_socket_cleans_the_socket_file() -> Result<(), Error> {
        let dir = std::env::temp_dir().join("temp-socket");
        let binding = Binding::FilePath(dir);
        let listener: Listener = binding.try_into()?;
        drop(listener);
        // create a second listener from the same path
        let dir = std::env::temp_dir().join("temp-socket");
        let binding = Binding::FilePath(dir);
        let listener: Listener = binding.try_into()?;
        drop(listener);
        Ok(())
    }
}
