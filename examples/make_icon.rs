//! Renders the program's icon (the lotus flower, see src/icon.rs) into assets/:
//! icon.ico for Windows (16 to 256 pixels, each rendered at its own size) and
//! icon.png (512 pixels) for macOS and the README.
//!
//! cargo run --release --example make_icon

use std::io::Write;

/// A PNG of packed RGB pixels, as 8-bit RGBA (what icons expect), compressed.
fn png_rgba(size: usize, rgb: &[u8]) -> Vec<u8> {
    fn crc32(data: &[u8], mut crc: u32) -> u32 {
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        crc
    }
    let mut raw = Vec::with_capacity(size * (4 * size + 1));
    for row in rgb.chunks(3 * size) {
        raw.push(0);
        row.chunks(3).for_each(|c| raw.extend_from_slice(&[c[0], c[1], c[2], 255]));
    }
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&(!crc32(data, crc32(kind, 0xFFFF_FFFF))).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(size as u32).to_be_bytes());
    ihdr.extend_from_slice(&(size as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit, RGBA
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

fn main() -> std::io::Result<()> {
    std::fs::create_dir_all("assets")?;
    // an .ico holding PNG images (Windows Vista and later)
    let sizes = [16usize, 24, 32, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| png_rgba(s, &algfrac::icon::lotus(s))).collect();
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (&s, img) in sizes.iter().zip(&images) {
        let side = if s >= 256 { 0 } else { s as u8 };
        ico.extend_from_slice(&[side, side, 0, 0, 1, 0, 32, 0]);
        ico.extend_from_slice(&(img.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += img.len();
    }
    images.iter().for_each(|img| ico.extend_from_slice(img));
    std::fs::File::create("assets/icon.ico")?.write_all(&ico)?;
    std::fs::write("assets/icon.png", png_rgba(512, &algfrac::icon::lotus(512)))?;
    println!("wrote assets/icon.ico ({} bytes) and assets/icon.png", ico.len());
    Ok(())
}
