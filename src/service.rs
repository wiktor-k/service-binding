use std::net::SocketAddr;
use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use super::Error;

#[derive(Debug, PartialEq)]
pub enum Binding<'a> {
    FileDescriptor(i32),
    FilePath(&'a str),
    Socket(SocketAddr),
}

#[derive(Debug)]
pub enum Listener {
    Unix(UnixListener),
    Tcp(TcpListener),
}

impl From<UnixListener> for Listener {
    fn from(listener: UnixListener) -> Self {
        while listener.set_nonblocking(true).is_err() {
            // retry WouldBlock errors
        }

        Listener::Unix(listener)
    }
}

impl From<TcpListener> for Listener {
    fn from(listener: TcpListener) -> Self {
        while listener.set_nonblocking(true).is_err() {
            // retry WouldBlock errors
        }

        Listener::Tcp(listener)
    }
}

impl<'a> std::convert::TryFrom<&'a str> for Binding<'a> {
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
            Ok(Binding::FilePath(file))
        } else if let Some(addr) = s.strip_prefix("tcp://") {
            let addr: SocketAddr = addr.parse()?;
            Ok(Binding::Socket(addr))
        } else {
            Err(Error)
        }
    }
}

impl<'a> TryFrom<Binding<'a>> for Listener {
    type Error = Error;

    fn try_from(value: Binding) -> Result<Self, Self::Error> {
        match value {
            Binding::FileDescriptor(descriptor) => {
                if descriptor != 3 {
                    return Err(Error);
                }

                use std::os::unix::io::FromRawFd;

                Ok(unsafe { UnixListener::from_raw_fd(descriptor) }.into())
            }
            Binding::FilePath(path) => {
                // ignore errors if the file does not exist
                let _ = std::fs::remove_file(path);
                Ok(UnixListener::bind(path)?.into())
            }
            Binding::Socket(socket) => Ok(std::net::TcpListener::bind(&socket)?.into()),
        }
    }
}

impl<'a> std::convert::TryFrom<&'a str> for Listener {
    type Error = Error;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        let binding: Binding = s.try_into()?;
        binding.try_into()
    }
}

impl std::str::FromStr for Listener {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let binding: Binding = s.try_into()?;
        binding.try_into()
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
        let binding = Binding::FilePath(&dir.to_str().unwrap());
        let listener: Listener = binding.try_into()?;
        drop(listener);
        // create a second listener from the same path
        let dir = std::env::temp_dir().join("temp-socket");
        let binding = Binding::FilePath(&dir.to_str().unwrap());
        let listener: Listener = binding.try_into()?;
        drop(listener);
        Ok(())
    }
}
