use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordVerifier, password_hash::PasswordHasher};

pub fn argon2_hash(password: &str) -> Result<String> {
    let argon2 = Argon2::default();
    let hash_salt = argon2.hash_password(password.as_bytes())?.to_string();

    Ok(hash_salt)
}

pub fn argon2_valid(password: &str, hash: &str) -> Result<()> {
    let parsed = PasswordHash::new(hash)?;

    match Argon2::default().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(()),
        Err(_) => Err(anyhow::anyhow!("password mismatch")),
    }
}
