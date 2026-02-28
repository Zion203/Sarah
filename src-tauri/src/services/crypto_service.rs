use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use zeroize::Zeroize;

use crate::error::AppError;

const MASTER_KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedData {
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

impl EncryptedData {
    pub fn to_compact(&self) -> String {
        format!("{}:{}", self.nonce_b64, self.ciphertext_b64)
    }

    pub fn from_compact(compact: &str) -> Result<Self, AppError> {
        let mut split = compact.splitn(2, ':');
        let nonce_b64 = split
            .next()
            .ok_or_else(|| AppError::Crypto("Missing nonce component".to_string()))?
            .to_string();
        let ciphertext_b64 = split
            .next()
            .ok_or_else(|| AppError::Crypto("Missing ciphertext component".to_string()))?
            .to_string();

        Ok(Self {
            nonce_b64,
            ciphertext_b64,
        })
    }
}

#[derive(Clone)]
pub struct CryptoService {
    service_name: String,
    entry_name: String,
    master_key: [u8; MASTER_KEY_BYTES],
}

impl CryptoService {
    pub fn new(app_bundle_id: &str) -> Result<Self, AppError> {
        let service_name = format!("{app_bundle_id}:master_key");
        let entry_name = "local-user".to_string();

        let entry = keyring::Entry::new(&service_name, &entry_name)?;
        let key = match entry.get_password() {
            Ok(value) => {
                let bytes = base64::engine::general_purpose::STANDARD.decode(value)?;
                if bytes.len() != MASTER_KEY_BYTES {
                    return Err(AppError::Crypto(
                        "Stored master key has invalid length".to_string(),
                    ));
                }
                let mut arr = [0u8; MASTER_KEY_BYTES];
                arr.copy_from_slice(&bytes);
                arr
            }
            Err(_) => {
                let mut bytes = [0u8; MASTER_KEY_BYTES];
                OsRng.fill_bytes(&mut bytes);
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                entry.set_password(&encoded)?;
                bytes
            }
        };

        Ok(Self {
            service_name,
            entry_name,
            master_key: key,
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, AppError> {
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|_| AppError::Crypto("Failed to initialize AES-256-GCM cipher".to_string()))?;

        let mut nonce_raw = [0u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce_raw);
        let nonce = Nonce::from_slice(&nonce_raw);

        let ciphertext = cipher.encrypt(nonce, plaintext)?;
        let nonce_b64 = base64::engine::general_purpose::STANDARD.encode(nonce_raw);
        let ciphertext_b64 = base64::engine::general_purpose::STANDARD.encode(ciphertext);

        Ok(EncryptedData {
            nonce_b64,
            ciphertext_b64,
        })
    }

    pub fn decrypt(&self, encrypted: &str) -> Result<Vec<u8>, AppError> {
        let compact = EncryptedData::from_compact(encrypted)?;
        let nonce_raw = base64::engine::general_purpose::STANDARD.decode(compact.nonce_b64)?;
        if nonce_raw.len() != NONCE_BYTES {
            return Err(AppError::Crypto("Invalid nonce length".to_string()));
        }
        let ciphertext =
            base64::engine::general_purpose::STANDARD.decode(compact.ciphertext_b64)?;

        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|_| AppError::Crypto("Failed to initialize AES-256-GCM cipher".to_string()))?;

        let plaintext = cipher.decrypt(Nonce::from_slice(&nonce_raw), ciphertext.as_ref())?;
        Ok(plaintext)
    }

    pub fn encrypt_to_compact(&self, plaintext: &[u8]) -> Result<String, AppError> {
        Ok(self.encrypt(plaintext)?.to_compact())
    }

    pub fn zeroize_after_use(data: &mut Vec<u8>) {
        data.zeroize();
    }

    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    pub fn entry_name(&self) -> &str {
        &self.entry_name
    }
}

impl Drop for CryptoService {
    fn drop(&mut self) {
        self.master_key.zeroize();
    }
}
