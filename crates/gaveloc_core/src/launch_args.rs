use base64::{engine::general_purpose, Engine as _};
use blowfish::cipher::{BlockEncrypt, KeyInit};
use blowfish::Blowfish;
use byteorder::LittleEndian;
use generic_array::GenericArray;

use crate::config::{Language, Region};
use crate::error::Error;

/// FFXIV's known Blowfish key for argument encryption.
/// This is public knowledge from reverse engineering the official launcher.
const ENCRYPTION_KEY: &[u8] = b"#:G$.,:5";

/// A session ID that has been encrypted for use in launch arguments.
/// Can only be created via `EncryptedSessionId::new()` which performs encryption.
#[derive(Debug, Clone)]
pub struct EncryptedSessionId(String);

impl EncryptedSessionId {
    /// Encrypts a raw session ID and returns the encrypted wrapper.
    pub fn new(raw_session_id: &str) -> Result<Self, Error> {
        let encrypted = encrypt_argument(raw_session_id)?;
        Ok(Self(encrypted))
    }

    /// Returns the encrypted session ID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Encrypts a string using Blowfish ECB mode with PKCS7 padding.
/// Returns a Base64-encoded result suitable for FFXIV's launcher protocol.
fn encrypt_argument(plain_text: &str) -> Result<String, Error> {
    let cipher = Blowfish::<LittleEndian>::new_from_slice(ENCRYPTION_KEY)
        .map_err(|e| Error::Encryption(format!("invalid key: {}", e)))?;

    let mut buffer = plain_text.as_bytes().to_vec();

    // PKCS7 padding
    let block_size = 8;
    let padding_len = block_size - (buffer.len() % block_size);
    buffer.resize(buffer.len() + padding_len, padding_len as u8);

    // Encrypt each 8-byte block (ECB mode)
    for chunk in buffer.chunks_exact_mut(block_size) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        chunk.copy_from_slice(&block);
    }

    Ok(general_purpose::STANDARD.encode(&buffer))
}

/// Parameters for building FFXIV launch arguments.
pub struct LaunchParams<'a> {
    pub session_id: &'a EncryptedSessionId,
    pub max_expansion: u32,
    pub game_version: &'a str,
    pub is_steam: bool,
    pub region: Region,
    pub language: Language,
}

/// Builds the command-line argument string for ffxiv_dx11.exe.
pub fn build_launch_args(params: &LaunchParams<'_>) -> String {
    format!(
        "DEV.DataPathType=1 \
         DEV.MaxEntitledExpansionID={} \
         DEV.TestSID={} \
         DEV.UseSqPack=1 \
         SYS.Region={} \
         language={} \
         ver={} \
         IsSteam={}",
        params.max_expansion,
        params.session_id.as_str(),
        params.region.as_id(),
        params.language.as_id(),
        params.game_version,
        if params.is_steam { 1 } else { 0 }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_encryption_is_deterministic() {
        let session_id = "test_session_id";
        let encrypted1 = EncryptedSessionId::new(session_id).unwrap();
        let encrypted2 = EncryptedSessionId::new(session_id).unwrap();

        assert_eq!(encrypted1.as_str(), encrypted2.as_str());
    }

    #[test]
    fn test_encryption_output_format() {
        let session_id = "test";
        let encrypted = EncryptedSessionId::new(session_id).unwrap();

        // Result should be valid base64
        let decoded = general_purpose::STANDARD.decode(encrypted.as_str());
        assert!(decoded.is_ok(), "Output should be valid Base64");

        // Encrypted output should not be empty
        assert!(!encrypted.as_str().is_empty());
    }

    #[rstest]
    #[case("", 8)]           // Empty string + 8 bytes padding
    #[case("a", 8)]          // 1 byte + 7 padding = 8
    #[case("test", 8)]       // 4 bytes + 4 padding = 8
    #[case("12345678", 16)]  // 8 bytes + 8 padding = 16
    fn test_encryption_block_sizes(#[case] input: &str, #[case] expected_decoded_len: usize) {
        let encrypted = EncryptedSessionId::new(input).unwrap();
        let decoded = general_purpose::STANDARD.decode(encrypted.as_str()).unwrap();
        assert_eq!(decoded.len(), expected_decoded_len);
    }

    #[rstest]
    #[case(Region::Japan, Language::Japanese, true, "SYS.Region=1", "language=0", "IsSteam=1")]
    #[case(Region::Europe, Language::English, true, "SYS.Region=3", "language=1", "IsSteam=1")]
    #[case(Region::NorthAmerica, Language::French, false, "SYS.Region=2", "language=3", "IsSteam=0")]
    #[case(Region::Europe, Language::German, false, "SYS.Region=3", "language=2", "IsSteam=0")]
    fn test_build_launch_args_variants(
        #[case] region: Region,
        #[case] language: Language,
        #[case] is_steam: bool,
        #[case] expected_region: &str,
        #[case] expected_lang: &str,
        #[case] expected_steam: &str,
    ) {
        let session_id = EncryptedSessionId::new("test").unwrap();
        let params = LaunchParams {
            session_id: &session_id,
            max_expansion: 5,
            game_version: "ver",
            is_steam,
            region,
            language,
        };
        let args = build_launch_args(&params);
        assert!(args.contains(expected_region));
        assert!(args.contains(expected_lang));
        assert!(args.contains(expected_steam));
    }

    #[test]
    fn test_build_launch_args_contains_required_fields() {
        let session_id = EncryptedSessionId::new("abc").unwrap();
        let params = LaunchParams {
            session_id: &session_id,
            max_expansion: 5,
            game_version: "2023.01.01.0000.0000",
            is_steam: true,
            region: Region::Europe,
            language: Language::English,
        };

        let args = build_launch_args(&params);

        assert!(args.contains("DEV.DataPathType=1"));
        assert!(args.contains("DEV.MaxEntitledExpansionID=5"));
        assert!(args.contains(&format!("DEV.TestSID={}", session_id.as_str())));
        assert!(args.contains("DEV.UseSqPack=1"));
        assert!(args.contains("ver=2023.01.01.0000.0000"));
    }

    #[test]
    fn test_launch_args_snapshot_steam() {
        let session_id = EncryptedSessionId::new("fixed_session").unwrap();
        let params = LaunchParams {
            session_id: &session_id,
            max_expansion: 5,
            game_version: "2024.01.01.0000.0000",
            is_steam: true,
            region: Region::Europe,
            language: Language::English,
        };
        insta::assert_yaml_snapshot!(build_launch_args(&params));
    }

    #[test]
    fn test_launch_args_snapshot_non_steam() {
        let session_id = EncryptedSessionId::new("fixed_session").unwrap();
        let params = LaunchParams {
            session_id: &session_id,
            max_expansion: 4,
            game_version: "2024.01.01.0000.0000",
            is_steam: false,
            region: Region::Japan,
            language: Language::Japanese,
        };
        insta::assert_yaml_snapshot!(build_launch_args(&params));
    }
}
