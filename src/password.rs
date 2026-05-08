use argon2::{
    password_hash::{rand_core::OsRng, Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

#[allow(dead_code)]
pub fn hash_password(password: &str) -> Result<String, PasswordHashError> {
    let salt = SaltString::generate(&mut OsRng);

    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
}

#[allow(dead_code)]
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, PasswordHashError> {
    let parsed_hash = PasswordHash::new(password_hash)?;

    match Argon2::default().verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(PasswordHashError::Password) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};

    #[test]
    fn hashes_and_verifies_a_password() {
        let password_hash = hash_password("correct horse battery staple")
            .expect("password hashing should succeed");

        let is_valid = verify_password("correct horse battery staple", &password_hash)
            .expect("password verification should succeed");

        assert!(is_valid);
    }

    #[test]
    fn rejects_the_wrong_password() {
        let password_hash =
            hash_password("correct horse battery staple").expect("password hashing should succeed");

        let is_valid =
            verify_password("tr0ub4dor&3", &password_hash).expect("password verification should succeed");

        assert!(!is_valid);
    }
}
