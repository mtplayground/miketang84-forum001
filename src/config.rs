use std::{env, error::Error, fmt, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub session_secret: String,
    pub bind_addr: SocketAddr,
    pub rust_log: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let database_url = required_var("DATABASE_URL")?;
        let session_secret = required_var("SESSION_SECRET")?;
        let bind_addr = parse_socket_addr("BIND_ADDR")?;
        let rust_log = required_var("RUST_LOG")?;

        Ok(Self {
            database_url,
            session_secret,
            bind_addr,
            rust_log,
        })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingVar {
        key: &'static str,
        source: env::VarError,
    },
    InvalidSocketAddr {
        key: &'static str,
        value: String,
        source: std::net::AddrParseError,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVar { key, .. } => write!(f, "missing required environment variable `{key}`"),
            Self::InvalidSocketAddr { key, value, .. } => {
                write!(f, "invalid socket address in `{key}`: `{value}`")
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingVar { source, .. } => Some(source),
            Self::InvalidSocketAddr { source, .. } => Some(source),
        }
    }
}

fn required_var(key: &'static str) -> Result<String, ConfigError> {
    env::var(key).map_err(|source| ConfigError::MissingVar { key, source })
}

fn parse_socket_addr(key: &'static str) -> Result<SocketAddr, ConfigError> {
    let value = required_var(key)?;

    value
        .parse::<SocketAddr>()
        .map_err(|source| ConfigError::InvalidSocketAddr { key, value, source })
}
