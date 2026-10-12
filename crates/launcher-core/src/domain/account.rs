use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;

/// Minecraft username accepted by offline mode: 3 to 16 characters of `[A-Za-z0-9_]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Username(String);

impl Username {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let name = raw.trim();
        let valid_len = (3..=16).contains(&name.chars().count());
        let valid_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if valid_len && valid_chars {
            Ok(Self(name.to_owned()))
        } else {
            Err(DomainError::InvalidUsername(raw.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Username {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Username> for String {
    fn from(value: Username) -> Self {
        value.0
    }
}

/// Player identity for offline mode. The UUID is derived from the name, as the vanilla server does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineAccount {
    username: Username,
}

impl OfflineAccount {
    pub fn new(username: Username) -> Self {
        Self { username }
    }

    pub fn username(&self) -> &Username {
        &self.username
    }

    /// `UUID.nameUUIDFromBytes("OfflinePlayer:<name>")` from the JDK: MD5 with version 3 and the IETF variant.
    pub fn uuid(&self) -> Uuid {
        let digest = Md5::digest(format!("OfflinePlayer:{}", self.username.as_str()).as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest);
        uuid::Builder::from_md5_bytes(bytes).into_uuid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_the_jdk_name_based_uuid() {
        // Verified with Python: md5(b"OfflinePlayer:Notch") with version 3 and IETF variant bits.
        let account = OfflineAccount::new(Username::parse("Notch").unwrap());
        assert_eq!(
            account.uuid().to_string(),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
        assert_eq!(account.uuid().get_version_num(), 3);
    }

    #[test]
    fn usernames_are_validated_and_trimmed() {
        assert_eq!(Username::parse("  Steve_01 ").unwrap().as_str(), "Steve_01");
        for bad in ["ab", "this_name_is_way_too_long", "bad name", "ñandú", ""] {
            assert!(Username::parse(bad).is_err(), "{bad} should be rejected");
        }
    }
}
