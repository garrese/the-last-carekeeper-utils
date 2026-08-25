//! Read-only compatibility layer for retoc's `oodle_loader` dependency.
//!
//! The application only extracts installed game assets. Decompression is
//! implemented in pure Rust by `oozextract`; compression is deliberately not
//! supported so this dependency cannot be used to create or modify game packs.

use std::io::Cursor;
use std::sync::OnceLock;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum Compressor {
    None = 3,
    Kraken = 8,
    Leviathan = 13,
    Mermaid = 9,
    Selkie = 11,
    Hydra = 12,
}

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum CompressionLevel {
    None = 0,
    SuperFast = 1,
    VeryFast = 2,
    Fast = 3,
    Normal = 4,
    Optimal1 = 5,
    Optimal2 = 6,
    Optimal3 = 7,
    Optimal4 = 8,
    Optimal5 = 9,
    HyperFast1 = -1,
    HyperFast2 = -2,
    HyperFast3 = -3,
    HyperFast4 = -4,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Oodle compression is disabled in this read-only application")]
    CompressionUnsupported,
}

pub struct Oodle;

impl Oodle {
    pub fn compress(
        &self,
        _input: &[u8],
        _compressor: Compressor,
        _compression_level: CompressionLevel,
    ) -> Result<Vec<u8>> {
        Err(Error::CompressionUnsupported)
    }

    pub fn decompress(&self, input: &[u8], output: &mut [u8]) -> isize {
        let mut extractor = oozextract::Extractor::new();
        let mut input = Cursor::new(input);
        match extractor.read(&mut input, output) {
            Ok(written) if written == output.len() => written as isize,
            Ok(_) => -1,
            Err(_) => -1,
        }
    }
}

static OODLE: OnceLock<Oodle> = OnceLock::new();

pub fn oodle() -> Result<&'static Oodle> {
    Ok(OODLE.get_or_init(|| Oodle))
}
