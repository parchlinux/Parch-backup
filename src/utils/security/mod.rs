use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use hkdf::Hkdf;
use sha2::Sha256;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum CryptoError {
    FileRead(io::Error),
    Encryption,
    Decryption,
    FileWrite(io::Error),
}

impl From<io::Error> for CryptoError {
    fn from(err: io::Error) -> CryptoError {
        CryptoError::FileWrite(err)
    }
}

/// Derives a 32-byte key from the given key using HKDF.
fn derive_key(key: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, key);
    let mut okm = [0u8; 32];
    hk.expand(&[], &mut okm)
        .expect("32 bytes is a valid length for HKDF output");
    okm
}

/// Encrypts a file using AES-GCM.
pub fn encrypt_file<P: AsRef<Path>>(file_path: P, key: &[u8]) -> Result<(), CryptoError> {
    let data = fs::read(file_path.as_ref()).map_err(CryptoError::FileRead)?;
    let derived_key = derive_key(key);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived_key));

    let nonce = GenericArray::from_slice(b"unique nonce"); // 12-bytes; unique per message
    let ciphertext = cipher
        .encrypt(nonce, Payload { msg: &data, aad: b"" })
        .map_err(|_| CryptoError::Encryption)?;

    fs::write(file_path.as_ref(), &ciphertext).map_err(CryptoError::FileWrite)?;
    Ok(())
}

/// Decrypts a file using AES-GCM.
pub fn decrypt_file<P: AsRef<Path>>(file_path: P, key: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let data = fs::read(file_path.as_ref()).map_err(CryptoError::FileRead)?;
    let derived_key = derive_key(key);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&derived_key));
    let nonce = GenericArray::from_slice(b"unique nonce");
    let plaintext = cipher
        .decrypt(nonce, Payload { msg: &data, aad: b"" })
        .map_err(|_| CryptoError::Decryption)?;
    Ok(plaintext)
}
