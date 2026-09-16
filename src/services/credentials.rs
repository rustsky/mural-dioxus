//! API keys live in the login Keychain and are sent only to their own provider.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyError { Invalid, Save, Remove }

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            KeyError::Invalid => "Enter a valid API key.",
            KeyError::Save => "The key couldn’t be saved to this Mac’s Keychain.",
            KeyError::Remove => "The key couldn’t be removed. Unlock this Mac’s Keychain and try again.",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Credential {
    service: &'static str,
    prefix: &'static str,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    debug_env: &'static str,
}

pub const OPENAI: Credential = Credential { service: "no.william.mural.openai", prefix: "sk-", debug_env: "MURAL_DEBUG_API_KEY" };
pub const OLLAMA: Credential = Credential { service: "chat.mural.ollama", prefix: "", debug_env: "MURAL_DEBUG_OLLAMA_KEY" };

const ACCOUNT: &str = "owner";

impl Credential {
    pub fn validate(&self, key: &str) -> Result<String, KeyError> {
        let value = key.trim();
        if !value.starts_with(self.prefix) || value.chars().count() < 20 || value.chars().any(char::is_whitespace) {
            return Err(KeyError::Invalid);
        }
        Ok(value.to_string())
    }

    pub fn read(&self) -> Option<String> {
        // Debug builds can exercise the request path without touching the Keychain.
        #[cfg(debug_assertions)]
        if let Ok(key) = std::env::var(self.debug_env) { return Some(key); }
        platform::read(self.service)
    }

    pub fn has_key(&self) -> bool { self.read().is_some() }

    pub fn save(&self, key: &str) -> Result<(), KeyError> {
        let value = self.validate(key)?;
        platform::save(self.service, &value)
    }

    pub fn delete(&self) -> Result<(), KeyError> { platform::delete(self.service) }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

    pub fn read(service: &str) -> Option<String> {
        get_generic_password(service, ACCOUNT).ok().and_then(|d| String::from_utf8(d).ok())
    }
    pub fn save(service: &str, value: &str) -> Result<(), KeyError> {
        set_generic_password(service, ACCOUNT, value.as_bytes()).map_err(|_| KeyError::Save)
    }
    pub fn delete(service: &str) -> Result<(), KeyError> {
        match delete_generic_password(service, ACCOUNT) {
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
    pub fn read(_service: &str) -> Option<String> { None }
    pub fn save(_service: &str, _value: &str) -> Result<(), KeyError> { Err(KeyError::Save) }
    pub fn delete(_service: &str) -> Result<(), KeyError> { Err(KeyError::Remove) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_keys() {
        assert!(OPENAI.validate("nope").is_err());
        assert!(OPENAI.validate("sk-short").is_err());
        assert!(OPENAI.validate("sk-abc def ghi jkl mno pqr").is_err());
        assert_eq!(OPENAI.validate("  sk-abcdefghijklmnopqrstu \n").unwrap(), "sk-abcdefghijklmnopqrstu");
        assert!(OLLAMA.validate("0123456789abcdef.0123456789abcdef").is_ok());
        assert!(OLLAMA.validate("short").is_err());
    }
}
