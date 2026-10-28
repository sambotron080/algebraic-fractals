//! Minimal PNG writers (8-bit RGB): uncompressed, with no dependencies (the command
//! line's), and compressed with miniz_oxide (the app's images and video frames).

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
    write_chunks(path.as_ref(), w, h, &z)
}

/// Like `write_png`, but compressed: each row stored as the difference from the pixel
/// to its left (PNG's Sub filter), then deflated. Star fields, mostly black, shrink to a
/// small fraction.
pub fn write_png_compressed(path: impl AsRef<Path>, w: usize, h: usize, rgb: &[u8]) -> io::Result<()> {
    let mut raw = Vec::with_capacity(h * (3 * w + 1));
    for row in rgb.chunks(3 * w) {
        raw.push(1); // filter: Sub
        raw.extend(row.iter().enumerate().map(|(i, &v)| if i < 3 { v } else { v.wrapping_sub(row[i - 3]) }));
    }
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
    write_chunks(path.as_ref(), w, h, &z)
}

fn write_chunks(path: &Path, w: usize, h: usize, z: &[u8]) -> io::Result<()> {
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
    chunk(b"IDAT", z)?;
    chunk(b"IEND", &[])?;
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compressed PNG decodes to the same pixels (checked by undoing our own Sub
    /// filter after inflating), and is much smaller for a mostly black image.
    #[test]
    fn compressed_png_round_trip() {
        let (w, h) = (64, 40);
        let rgb: Vec<u8> = (0..w * h * 3).map(|i| if i % 97 < 5 { (i * 7 % 256) as u8 } else { 0 }).collect();
        let dir = std::env::temp_dir();
        let (a, b) = (dir.join("algfrac_png_plain.png"), dir.join("algfrac_png_z.png"));
        write_png(&a, w, h, &rgb).unwrap();
        write_png_compressed(&b, w, h, &rgb).unwrap();
        let bytes = std::fs::read(&b).unwrap();
        assert!(bytes.len() * 3 < std::fs::metadata(&a).unwrap().len() as usize);
        // IDAT starts after the signature (8) and IHDR (8 + 13 + 4)
        let len = u32::from_be_bytes(bytes[33..37].try_into().unwrap()) as usize;
        assert_eq!(&bytes[37..41], b"IDAT");
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&bytes[41..41 + len]).unwrap();
        let mut back = Vec::new();
        for row in raw.chunks(3 * w + 1) {
            assert_eq!(row[0], 1);
            let start = back.len();
            for (i, &d) in row[1..].iter().enumerate() {
                let left = if i < 3 { 0 } else { back[start + i - 3] };
                back.push(d.wrapping_add(left));
            }
        }
        assert_eq!(back, rgb);
        let _ = (std::fs::remove_file(a), std::fs::remove_file(b));
    }
}
