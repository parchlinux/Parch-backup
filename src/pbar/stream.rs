use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use argon2::Argon2;
use std::io::{self, Read, Write};

pub const CHUNK_SIZE: usize = 64 * 1024; // 64 KB

pub fn derive_argon2_key(passphrase: &[u8], salt: &[u8; 16]) -> io::Result<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase, salt, &mut key)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Argon2 KDF failed: {}", e)))?;
    Ok(key)
}

pub fn derive_chunk_nonce(base_nonce: &[u8; 12], chunk_index: u64) -> GenericArray<u8, aes_gcm::aead::consts::U12> {
    let mut nonce = *base_nonce;
    let idx_bytes = chunk_index.to_be_bytes();
    for i in 0..8 {
        nonce[4 + i] ^= idx_bytes[i];
    }
    GenericArray::clone_from_slice(&nonce)
}

pub fn construct_ad(chunk_index: u64) -> Vec<u8> {
    let mut ad = Vec::with_capacity(12);
    ad.extend_from_slice(&chunk_index.to_be_bytes());
    ad.extend_from_slice(b"PBAR");
    ad
}

/// Writer wrapper that streams plaintext, encrypts in 64KB AEAD chunks, and writes to underlying stream.
pub struct PbarChunkWriter<W: Write> {
    writer: W,
    cipher: Option<Aes256Gcm>,
    base_nonce: [u8; 12],
    chunk_index: u64,
    buffer: Vec<u8>,
    bytes_written: u64,
}

impl<W: Write> PbarChunkWriter<W> {
    pub fn new(writer: W, key: Option<[u8; 32]>, base_nonce: [u8; 12]) -> Self {
        let cipher = key.map(|k| Aes256Gcm::new(GenericArray::from_slice(&k)));
        Self {
            writer,
            cipher,
            base_nonce,
            chunk_index: 0,
            buffer: Vec::with_capacity(CHUNK_SIZE),
            bytes_written: 0,
        }
    }

    pub fn total_bytes_written(&self) -> u64 {
        self.bytes_written
    }

    fn flush_chunk(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        if let Some(ref cipher) = self.cipher {
            let nonce = derive_chunk_nonce(&self.base_nonce, self.chunk_index);
            let ad = construct_ad(self.chunk_index);
            let ciphertext = cipher
                .encrypt(&nonce, Payload { msg: &self.buffer, aad: &ad })
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "AES-GCM chunk encryption failed"))?;

            let chunk_len = ciphertext.len() as u32;
            self.writer.write_all(&chunk_len.to_be_bytes())?;
            self.writer.write_all(&ciphertext)?;
        } else {
            self.writer.write_all(&self.buffer)?;
        }

        self.bytes_written += self.buffer.len() as u64;
        self.chunk_index += 1;
        self.buffer.clear();
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.flush_chunk()?;
        self.writer.flush()?;
        Ok(self.writer)
    }
}

impl<W: Write> Write for PbarChunkWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < buf.len() {
            let space = CHUNK_SIZE - self.buffer.len();
            let to_copy = (buf.len() - written).min(space);
            self.buffer.extend_from_slice(&buf[written..written + to_copy]);
            written += to_copy;

            if self.buffer.len() == CHUNK_SIZE {
                self.flush_chunk()?;
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_chunk()?;
        self.writer.flush()
    }
}

/// Reader wrapper that reads 64KB AEAD chunks from underlying stream and decrypts into plaintext stream.
pub struct PbarChunkReader<R: Read> {
    reader: R,
    cipher: Option<Aes256Gcm>,
    base_nonce: [u8; 12],
    chunk_index: u64,
    buffer: Vec<u8>,
    buffer_offset: usize,
    eof: bool,
}

impl<R: Read> PbarChunkReader<R> {
    pub fn new(reader: R, key: Option<[u8; 32]>, base_nonce: [u8; 12]) -> Self {
        let cipher = key.map(|k| Aes256Gcm::new(GenericArray::from_slice(&k)));
        Self {
            reader,
            cipher,
            base_nonce,
            chunk_index: 0,
            buffer: Vec::new(),
            buffer_offset: 0,
            eof: false,
        }
    }

    fn read_next_chunk(&mut self) -> io::Result<bool> {
        if self.eof {
            return Ok(false);
        }

        if let Some(ref cipher) = self.cipher {
            let mut len_bytes = [0u8; 4];
            match self.reader.read_exact(&mut len_bytes) {
                Ok(_) => {},
                Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    self.eof = true;
                    return Ok(false);
                }
                Err(e) => return Err(e),
            }

            let chunk_len = u32::from_be_bytes(len_bytes) as usize;
            let mut ciphertext = vec![0u8; chunk_len];
            self.reader.read_exact(&mut ciphertext)?;

            let nonce = derive_chunk_nonce(&self.base_nonce, self.chunk_index);
            let ad = construct_ad(self.chunk_index);

            let plaintext = cipher
                .decrypt(&nonce, Payload { msg: &ciphertext, aad: &ad })
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "AES-GCM decryption failed or bad password/tag"))?;

            self.buffer = plaintext;
            self.buffer_offset = 0;
            self.chunk_index += 1;
            Ok(true)
        } else {
            let mut raw_buf = vec![0u8; CHUNK_SIZE];
            let n = self.reader.read(&mut raw_buf)?;
            if n == 0 {
                self.eof = true;
                return Ok(false);
            }
            raw_buf.truncate(n);
            self.buffer = raw_buf;
            self.buffer_offset = 0;
            Ok(true)
        }
    }
}

impl<R: Read> Read for PbarChunkReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.buffer_offset >= self.buffer.len() {
            if !self.read_next_chunk()? {
                return Ok(0);
            }
        }

        let available = self.buffer.len() - self.buffer_offset;
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&self.buffer[self.buffer_offset..self.buffer_offset + to_read]);
        self.buffer_offset += to_read;
        Ok(to_read)
    }
}
