//! The OpenAI key lives in the login Keychain and is sent only to OpenAI.

const SERVICE: &str = "no.william.mural.openai";
const ACCOUNT: &str = "owner";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyError { Invalid, Save, Remove }

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            KeyError::Invalid => "Enter a valid OpenAI API key.",
            KeyError::Save => "The key couldn’t be saved to this Mac’s Keychain.",
            KeyError::Remove => "The key couldn’t be removed. Unlock this Mac’s Keychain and try again.",
        })
    }
}

pub fn validate(key: &str) -> Result<String, KeyError> {
    let value = key.trim();
    if !value.starts_with("sk-") || value.chars().count() < 20 || value.chars().any(char::is_whitespace) {
        return Err(KeyError::Invalid);
    }
    Ok(value.to_string())
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

    pub fn read() -> Option<String> {
        get_generic_password(SERVICE, ACCOUNT).ok().and_then(|d| String::from_utf8(d).ok())
    }
    pub fn save(key: &str) -> Result<(), KeyError> {
        let value = validate(key)?;
        set_generic_password(SERVICE, ACCOUNT, value.as_bytes()).map_err(|_| KeyError::Save)
    }
    pub fn delete() -> Result<(), KeyError> {
        match delete_generic_password(SERVICE, ACCOUNT) {
            Ok(()) => Ok(()),
            // errSecItemNotFound
            Err(e) if e.code() == -25300 => Ok(()),
            Err(_) => Err(KeyError::Remove),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;
    pub fn read() -> Option<String> { std::env::var("OPENAI_API_KEY").ok() }
    pub fn save(key: &str) -> Result<(), KeyError> { validate(key).map(|_| ()).and(Err(KeyError::Save)) }
    pub fn delete() -> Result<(), KeyError> { Err(KeyError::Remove) }
}

pub use platform::{delete, save};

pub fn read() -> Option<String> {
    // Debug builds can exercise the request path without touching the Keychain.
    #[cfg(debug_assertions)]
    if let Ok(key) = std::env::var("MURAL_DEBUG_API_KEY") { return Some(key); }
    platform::read()
}

pub fn has_key() -> bool { read().is_some() }

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_malformed_keys() {
        assert!(super::validate("nope").is_err());
        assert!(super::validate("sk-short").is_err());
        assert!(super::validate("sk-abc def ghi jkl mno pqr").is_err());
        assert_eq!(super::validate("  sk-abcdefghijklmnopqrstu \n").unwrap(), "sk-abcdefghijklmnopqrstu");
    }
}
