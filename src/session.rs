use std::{convert::TryFrom, error::Error, fmt};

use tower_sessions::cookie::Key;

const SESSION_SECRET_MIN_LENGTH: usize = 64;

#[derive(Debug)]
pub struct SessionSecretError {
    actual_len: usize,
}

impl fmt::Display for SessionSecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SESSION_SECRET must be at least {SESSION_SECRET_MIN_LENGTH} bytes for encrypted sessions; got {} bytes",
            self.actual_len
        )
    }
}

impl Error for SessionSecretError {}

pub fn session_encryption_key(session_secret: &str) -> Result<Key, SessionSecretError> {
    let secret_bytes = session_secret.as_bytes();

    Key::try_from(secret_bytes).map_err(|_| SessionSecretError {
        actual_len: secret_bytes.len(),
    })
}
