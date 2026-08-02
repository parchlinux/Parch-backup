use std::io::{self, Read, Write};

pub const PBAR_MAGIC: &[u8; 4] = b"PBAR";
pub const PBAR_VERSION: u16 = 0x0001; // v1.2 specification

// Feature Flags Bitfield
pub const FLAG_IS_ENCRYPTED: u16 = 1 << 0;
pub const FLAG_IS_COMPRESSED: u16 = 1 << 1;
pub const FLAG_COMPRESS_ZSTD: u16 = 1 << 2; // 00 = Gzip, 01 = Zstd
pub const FLAG_ENCRYPT_CHACHA: u16 = 1 << 4; // 00 = AES-256-GCM, 01 = ChaCha20
pub const FLAG_ENCRYPTED_MANIFEST: u16 = 1 << 6;
pub const FLAG_IS_SIGNED: u16 = 1 << 8;

#[derive(Debug, Clone)]
pub struct PbarHeader {
    pub version: u16,
    pub feature_flags: u16,
    pub salt: [u8; 16],
    pub base_nonce: [u8; 12],
    pub manifest_size: u32,
    pub signature_block: Option<PbarSignatureBlock>,
}

#[derive(Debug, Clone)]
pub struct PbarSignatureBlock {
    pub sig_type: u8,
    pub public_key: [u8; 32],
    pub signature: [u8; 64],
}

impl PbarHeader {
    pub fn new(is_encrypted: bool, is_compressed: bool, manifest_size: u32) -> Self {
        use rand::RngCore;

        let mut salt = [0u8; 16];
        let mut base_nonce = [0u8; 12];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut salt);
        rng.fill_bytes(&mut base_nonce);

        let mut feature_flags = 0u16;
        if is_encrypted {
            feature_flags |= FLAG_IS_ENCRYPTED | FLAG_ENCRYPTED_MANIFEST;
        }
        if is_compressed {
            feature_flags |= FLAG_IS_COMPRESSED;
        }

        Self {
            version: PBAR_VERSION,
            feature_flags,
            salt,
            base_nonce,
            manifest_size,
            signature_block: None,
        }
    }

    pub fn is_encrypted(&self) -> bool {
        (self.feature_flags & FLAG_IS_ENCRYPTED) != 0
    }

    pub fn is_compressed(&self) -> bool {
        (self.feature_flags & FLAG_IS_COMPRESSED) != 0
    }

    pub fn is_signed(&self) -> bool {
        (self.feature_flags & FLAG_IS_SIGNED) != 0
    }

    pub fn write_to<W: Write>(&self, mut writer: W) -> io::Result<()> {
        writer.write_all(PBAR_MAGIC)?;
        writer.write_all(&self.version.to_be_bytes())?;
        writer.write_all(&self.feature_flags.to_be_bytes())?;
        writer.write_all(&self.salt)?;
        writer.write_all(&self.base_nonce)?;
        writer.write_all(&self.manifest_size.to_be_bytes())?;

        if let Some(sig) = &self.signature_block {
            writer.write_all(&[sig.sig_type])?;
            writer.write_all(&sig.public_key)?;
            writer.write_all(&sig.signature)?;
        }

        Ok(())
    }

    pub fn read_from<R: Read>(mut reader: R) -> io::Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != PBAR_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid PBAR magic header. File is not a valid PBAR container.",
            ));
        }

        let mut version_bytes = [0u8; 2];
        reader.read_exact(&mut version_bytes)?;
        let version = u16::from_be_bytes(version_bytes);

        let mut flags_bytes = [0u8; 2];
        reader.read_exact(&mut flags_bytes)?;
        let feature_flags = u16::from_be_bytes(flags_bytes);

        let mut salt = [0u8; 16];
        reader.read_exact(&mut salt)?;

        let mut base_nonce = [0u8; 12];
        reader.read_exact(&mut base_nonce)?;

        let mut size_bytes = [0u8; 4];
        reader.read_exact(&mut size_bytes)?;
        let manifest_size = u32::from_be_bytes(size_bytes);

        let signature_block = if (feature_flags & FLAG_IS_SIGNED) != 0 {
            let mut sig_type = [0u8; 1];
            reader.read_exact(&mut sig_type)?;
            let mut public_key = [0u8; 32];
            reader.read_exact(&mut public_key)?;
            let mut signature = [0u8; 64];
            reader.read_exact(&mut signature)?;
            Some(PbarSignatureBlock {
                sig_type: sig_type[0],
                public_key,
                signature,
            })
        } else {
            None
        };

        Ok(Self {
            version,
            feature_flags,
            salt,
            base_nonce,
            manifest_size,
            signature_block,
        })
    }
}
