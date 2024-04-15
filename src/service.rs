use std::env::var;
use std::net::SocketAddr;
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use super::Error;

const SD_LISTEN_FDS_START: i32 = 3;

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
#[derive(Debug, PartialEq, Eq, Clone)]
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

    /// Windows Named Pipe.
    NamedPipe(std::ffi::OsString),
}

impl From<PathBuf> for Binding {
    fn from(value: PathBuf) -> Self {
        Binding::FilePath(value)
    }
}

impl From<SocketAddr> for Binding {
    fn from(value: SocketAddr) -> Self {
        Binding::Socket(value)
    }
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
/// let binding: Binding = "tcp://127.0.0.1:8080".parse().unwrap();
/// let listener = binding.try_into().unwrap();
/// assert!(matches!(listener, Listener::Tcp(_)));
/// ```
#[derive(Debug)]
pub enum Listener {
    /// Listener for a Unix domain socket.
    #[cfg(unix)]
    Unix(UnixListener),

    /// Listener for a TCP socket.
    Tcp(TcpListener),

    /// Named Pipe.
    NamedPipe(std::ffi::OsString),
}

#[cfg(unix)]
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
        if let Some(name) = s.strip_prefix("fd://") {
            if name.is_empty() {
                if let Ok(fds) = var("LISTEN_FDS") {
                    let fds: i32 = fds.parse()?;

                    // we support only one socket for now
                    if fds != 1 {
                        return Err(Error::DescriptorOutOfRange(fds));
                    }

                    return Ok(Binding::FileDescriptor(SD_LISTEN_FDS_START));
                } else {
                    return Err(Error::DescriptorsMissing);
                }
            }
            if let Ok(fd) = name.parse() {
                return Ok(Binding::FileDescriptor(fd));
            }
            #[cfg(target_os = "macos")]
            {
                let fds = raunch::activate_socket(name).map_err(|_| Error::DescriptorsMissing)?;
                if fds.len() == 1 {
                    Ok(Binding::FileDescriptor(fds[0]))
                } else {
                    Err(Error::DescriptorOutOfRange(fds.len() as i32))
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                if let (Ok(names), Ok(fds)) = (var("LISTEN_FDNAMES"), var("LISTEN_FDS")) {
                    let fds: usize = fds.parse()?;
                    for (fd_index, fd_name) in names.split(':').enumerate() {
                        if fd_name == name && fd_index < fds {
                            return Ok(Binding::FileDescriptor(
                                SD_LISTEN_FDS_START + fd_index as i32,
                            ));
                        }
                    }
                }
                Err(Error::DescriptorsMissing)
            }
        } else if let Some(file) = s.strip_prefix("unix://") {
            Ok(Binding::FilePath(file.into()))
        } else if let Some(file) = s.strip_prefix("npipe://") {
            if let Some('.' | '/' | '\\') = file.chars().next() {
                Ok(Binding::NamedPipe(file.replace('/', "\\").into()))
            } else {
                Ok(Binding::NamedPipe(format!(r"\\.\pipe\{file}").into()))
            }
        } else if let Some(addr) = s.strip_prefix("tcp://") {
            let addr: SocketAddr = addr.parse()?;
            Ok(Binding::Socket(addr))
        } else if s.starts_with(r"\\") {
            Ok(Binding::NamedPipe(s.into()))
        } else {
            Err(Error::UnsupportedScheme)
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
            #[cfg(unix)]
            Binding::FileDescriptor(descriptor) => {
                use std::os::unix::io::FromRawFd;

                Ok(unsafe { UnixListener::from_raw_fd(descriptor) }.into())
            }
            #[cfg(unix)]
            Binding::FilePath(path) => {
                // ignore errors if the file does not exist
                let _ = std::fs::remove_file(&path);
                Ok(UnixListener::bind(path)?.into())
            }
            Binding::Socket(socket) => Ok(std::net::TcpListener::bind(socket)?.into()),
            Binding::NamedPipe(pipe) => Ok(Listener::NamedPipe(pipe)),
            #[cfg(not(unix))]
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                Error::UnsupportedScheme,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use serial_test::serial;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    #[serial]
    fn parse_fd() -> TestResult {
        std::env::set_var("LISTEN_FDS", "1");
        let binding = "fd://".parse()?;
        assert_eq!(Binding::FileDescriptor(3), binding);

        let result: Result<Listener, _> = binding.try_into();

        // UnixListener is supported only on Unix platforms
        assert_eq!(cfg!(unix), result.is_ok());

        Ok(())
    }

    #[test]
    // on non-macOS systems this reads environment variables
    #[cfg(not(target_os = "macos"))]
    #[serial]
    fn parse_fd_named() -> TestResult {
        std::env::set_var("LISTEN_FDS", "2");
        std::env::set_var("LISTEN_FDNAMES", "other:service-name");
        let binding = "fd://service-name".parse()?;
        assert_eq!(Binding::FileDescriptor(4), binding);
        std::env::remove_var("LISTEN_FDNAMES");

        Ok(())
    }

    #[test]
    // on macOS the test will attempt launchd system activation but since
    // the plist file is not present it will fail
    #[cfg(target_os = "macos")]
    #[serial]
    fn parse_fd_named() -> TestResult {
        assert!(matches!(
            Binding::from_str("fd://service-name"),
            Err(Error::DescriptorsMissing)
        ));

        Ok(())
    }

    #[test]
    #[serial]
    fn parse_fd_bad() -> TestResult {
        std::env::set_var("LISTEN_FDS", "1"); // should be "2"
        std::env::set_var("LISTEN_FDNAMES", "other:service-name");
        assert!(matches!(
            Binding::from_str("fd://service-name"),
            Err(Error::DescriptorsMissing)
        ));
        std::env::remove_var("LISTEN_FDNAMES");

        Ok(())
    }

    #[test]
    #[serial]
    fn parse_fd_explicit() -> TestResult {
        let binding = "fd://56".parse()?;
        assert_eq!(Binding::FileDescriptor(56), binding);

        let result: Result<Listener, _> = binding.try_into();

        // UnixListener is supported only on Unix platforms
        assert_eq!(cfg!(unix), result.is_ok());

        Ok(())
    }

    #[test]
    #[serial]
    fn parse_fd_fail_unsupported_fds_count() -> TestResult {
        std::env::set_var("LISTEN_FDS", "3");
        assert!(matches!(
            Binding::from_str("fd://"),
            Err(Error::DescriptorOutOfRange(3))
        ));
        Ok(())
    }

    #[test]
    #[serial]
    fn parse_fd_fail_not_a_number() -> TestResult {
        std::env::set_var("LISTEN_FDS", "3a");
        assert!(matches!(
            Binding::from_str("fd://"),
            Err(Error::BadDescriptor(_))
        ));
        Ok(())
    }

    #[test]
    #[serial]
    fn parse_fd_fail() -> TestResult {
        std::env::remove_var("LISTEN_FDS");
        assert!(matches!(
            Binding::from_str("fd://"),
            Err(Error::DescriptorsMissing)
        ));
        Ok(())
    }

    #[test]
    fn parse_unix() -> TestResult {
        let binding = "unix:///tmp/test".try_into()?;
        assert_eq!(Binding::FilePath("/tmp/test".into()), binding);

        let result: Result<Listener, _> = binding.try_into();
        // UnixListener is supported only on Unix platforms
        if cfg!(unix) {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }

        Ok(())
    }

    #[test]
    fn parse_tcp() -> TestResult {
        let binding = "tcp://127.0.0.1:8080".try_into()?;
        assert_eq!(Binding::Socket(([127, 0, 0, 1], 8080).into()), binding);
        let _: Listener = binding.try_into()?;
        Ok(())
    }

    #[test]
    fn parse_tcp_fail() -> TestResult {
        assert!(matches!(
            Binding::try_from("tcp://:8080"),
            Err(Error::BadAddress(_))
        ));
        Ok(())
    }

    #[test]
    fn parse_pipe() -> TestResult {
        let binding = r"\\.\pipe\test".try_into()?;
        assert_eq!(Binding::NamedPipe(r"\\.\pipe\test".into()), binding);
        let _: Listener = binding.try_into()?;
        Ok(())
    }

    #[test]
    fn parse_pipe_short() -> TestResult {
        let binding = r"npipe://test".try_into()?;
        assert_eq!(Binding::NamedPipe(r"\\.\pipe\test".into()), binding);
        let _: Listener = binding.try_into()?;
        Ok(())
    }

    #[test]
    fn parse_pipe_long() -> TestResult {
        let binding = r"npipe:////./pipe/test".try_into()?;
        assert_eq!(Binding::NamedPipe(r"\\.\pipe\test".into()), binding);
        let _: Listener = binding.try_into()?;
        Ok(())
    }

    #[test]
    fn parse_pipe_fail() -> TestResult {
        assert!(matches!(
            Binding::try_from(r"\test"),
            Err(Error::UnsupportedScheme)
        ));
        Ok(())
    }

    #[test]
    fn parse_unknown_fail() -> TestResult {
        assert!(matches!(
            Binding::try_from("unknown://test"),
            Err(Error::UnsupportedScheme)
        ));
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    #[serial]
    fn test_bad_tcp_listener() -> TestResult {
        use std::os::unix::io::FromRawFd;

        let bad_file_descriptor = 41;
        let listener = unsafe { TcpListener::from_raw_fd(bad_file_descriptor) };

        // This will trigger Bad File Descriptor errors during conversion
        // in the WouldBlock loop.
        let _listener: Listener = listener.into();
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    #[serial]
    fn listen_on_socket_cleans_the_socket_file() -> TestResult {
        let dir = std::env::temp_dir().join("temp-socket");
        let binding = Binding::FilePath(dir);
        let listener: Listener = binding.try_into().unwrap();
        drop(listener);
        // create a second listener from the same path
        let dir = std::env::temp_dir().join("temp-socket");
        let binding = Binding::FilePath(dir);
        let listener: Listener = binding.try_into().unwrap();
        drop(listener);
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn convert_from_pathbuf() {
        let path = std::path::PathBuf::from("/tmp");
        let binding: Binding = path.into();
        assert!(matches!(binding, Binding::FilePath(_)));
    }

    #[test]
    fn convert_from_socket() {
        let socket: SocketAddr = ([127, 0, 0, 1], 8080).into();
        let binding: Binding = socket.into();
        assert!(matches!(binding, Binding::Socket(_)));
    }
}
