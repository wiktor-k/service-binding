use super::Error;

#[derive(Debug, PartialEq)]
pub enum Endpoint {
    Http(String),
    Unix(String, Option<String>),
}

impl std::str::FromStr for Endpoint {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            Ok(Endpoint::Http(s.into()))
        } else if let Some(path) = s.strip_prefix("unix://") {
            Ok(Endpoint::Unix(path.into(), None))
        } else {
            Err(Error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http() {
        let endpoint: Endpoint = "http://localhost".parse().unwrap();
        assert_eq!(endpoint, Endpoint::Http("http://localhost".into()));
    }

    #[test]
    fn parse_https() {
        let endpoint: Endpoint = "https://localhost".parse().unwrap();
        assert_eq!(endpoint, Endpoint::Http("https://localhost".into()));
    }

    #[test]
    fn parse_unix() {
        let endpoint: Endpoint = "unix:///tmp/socket".parse().unwrap();
        assert_eq!(endpoint, Endpoint::Unix("/tmp/socket".into(), None));
    }
}