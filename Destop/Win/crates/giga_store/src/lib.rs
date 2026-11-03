//! `giga_store` — Crash-Safe State Journal, Pre-Allocated Random-Access File Assembler,
//! and Native NIST FIPS-180-4 Streaming SHA-256 Integrity Verifier.

use giga_scheduler::ByteRangeChunk;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Persistent status of a multi-path transfer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferSessionState {
    Queued,
    Active,
    Paused,
    VerifyingIntegrity,
    Completed,
    Failed,
}

/// Persistent crash-recovery manifest written alongside an active download (`<file>.gigachad.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferManifest {
    pub session_id: String,
    pub url: String,
    pub output_path: PathBuf,
    pub total_bytes: u64,
    pub supports_range: bool,
    pub state: TransferSessionState,
    pub chunks: Vec<ByteRangeChunk>,
    pub sha256_hex: Option<String>,
}

impl TransferManifest {
    pub fn journal_path_for(output_path: &Path) -> PathBuf {
        let mut p = output_path.as_os_str().to_owned();
        p.push(".gigachad.json");
        PathBuf::from(p)
    }

    pub fn save_atomic(&self) -> std::io::Result<()> {
        let journal_path = Self::journal_path_for(&self.output_path);
        let tmp_path = journal_path.with_extension("json.tmp");
        let data = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&tmp_path, data)?;
        std::fs::rename(&tmp_path, &journal_path)?;
        Ok(())
    }

    pub fn load_if_exists(output_path: &Path) -> Option<Self> {
        let journal_path = Self::journal_path_for(output_path);
        let bytes = std::fs::read(journal_path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn remove_journal(output_path: &Path) {
        let journal_path = Self::journal_path_for(output_path);
        let _ = std::fs::remove_file(journal_path);
    }
}

/// Thread-safe random-access file writer that allows multiple bound interface workers to write
/// disjoint `[start_byte..=end_byte]` slices concurrently without buffering the file in RAM.
#[derive(Clone)]
pub struct RandomAccessFileAssembler {
    file: Arc<Mutex<File>>,
    path: PathBuf,
}

impl RandomAccessFileAssembler {
    /// Opens or creates `path` and pre-allocates `total_bytes` on disk if known (`> 0`).
    pub fn open_or_create(path: &Path, total_bytes: u64, resume: bool) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(!resume)
            .open(path)?;

        if total_bytes > 0 && !resume {
            file.set_len(total_bytes)?;
        }

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            path: path.to_path_buf(),
        })
    }

    /// Writes `data` at exact byte `offset` in the file.
    pub fn write_at(&self, offset: u64, data: &[u8]) -> std::io::Result<()> {
        let mut guard = self
            .file
            .lock()
            .map_err(|_| std::io::Error::other("File assembler mutex poisoned"))?;
        guard.seek(SeekFrom::Start(offset))?;
        guard.write_all(data)?;
        Ok(())
    }

    /// Flushes OS buffers to disk.
    pub fn sync_all(&self) -> std::io::Result<()> {
        let guard = self
            .file
            .lock()
            .map_err(|_| std::io::Error::other("File assembler mutex poisoned"))?;
        guard.sync_all()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Streaming NIST FIPS-180-4 SHA-256 implementation (zero external crate dependencies).
pub struct Sha256Hasher {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Default for Sha256Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256Hasher {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.buffer_len > 0 {
            let take = (64 - self.buffer_len).min(data.len());
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&data[..take]);
            self.buffer_len += take;
            data = &data[take..];
            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress_block(&block);
                self.buffer_len = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress_block(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffer_len = data.len();
        }
    }

    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len.wrapping_mul(8);
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            for b in &mut self.buffer[self.buffer_len..64] {
                *b = 0;
            }
            let block = self.buffer;
            self.compress_block(&block);
            self.buffer_len = 0;
        }
        for b in &mut self.buffer[self.buffer_len..56] {
            *b = 0;
        }
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress_block(&block);

        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn compress_block(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

/// Computes the SHA-256 hex digest of an in-memory byte slice.
pub fn sha256_bytes_hex(data: &[u8]) -> String {
    let mut hasher = Sha256Hasher::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut hex_str = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex_str, "{:02x}", b);
    }
    hex_str
}

/// Computes the streaming SHA-256 hex digest of a file on disk using a 64 KB buffer.
pub fn compute_file_sha256_hex(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut hex_str = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex_str, "{:02x}", b);
    }
    Ok(hex_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nist_sha256_vector() {
        // NIST test vector for "abc"
        let digest = sha256_bytes_hex(b"abc");
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
