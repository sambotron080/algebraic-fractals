//! Minimal PNG writer (8-bit RGB, uncompressed deflate), so no dependencies are needed.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

fn crc32(data: &[u8], mut crc: u32) -> u32 {
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    crc
}

/// Writes packed RGB pixels (row-major, top row first) as a PNG.
pub fn write_png(path: impl AsRef<Path>, w: usize, h: usize, rgb: &[u8]) -> io::Result<()> {
    let mut raw = Vec::with_capacity(h * (3 * w + 1));
    for row in rgb.chunks(3 * w) {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01]; // zlib header
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, blk) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8); // BFINAL, BTYPE = stored
        z.extend_from_slice(&(blk.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(blk.len() as u16)).to_le_bytes());
        z.extend_from_slice(blk);
    }
    let (mut a, mut b) = (1u32, 0u32); // adler32
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut f = BufWriter::new(File::create(path)?);
    f.write_all(b"\x89PNG\r\n\x1a\n")?;
    let mut chunk = |kind: &[u8], data: &[u8]| -> io::Result<()> {
        f.write_all(&(data.len() as u32).to_be_bytes())?;
        f.write_all(kind)?;
        f.write_all(data)?;
        f.write_all(&(!crc32(data, crc32(kind, 0xFFFF_FFFF))).to_be_bytes())
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, RGB
    chunk(b"IHDR", &ihdr)?;
    chunk(b"IDAT", &z)?;
    chunk(b"IEND", &[])?;
    f.flush()
}
