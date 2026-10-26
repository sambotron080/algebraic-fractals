//! .algf files: a render's raw data (every pixel's residual and best polynomial), so
//! it can be coloured again or shown without searching again.
//!
//! Binary layout (little-endian):
//!   "ALGF", u32 version=2, u32 width, u32 height, u32 order, i32 maxcoef,
//!   i32 reserved (= 1; once a minimum |c|), u32 coefset (index into CoefSet::ALL:
//!   0 ints, 1 primes, 2 fibonacci, 3 odd, 4 triangular, 5 squares, 6 cubes, 7 pow2),
//!   u32 flags (bit 0 legacy, bit 1 zero not allowed, bit 2 c0 = 0 allowed),
//!   f64 re_axis[width], f64 im_axis[height] (top row first),
//!   f64 residual[height*width] (row-major, top row first),
//!   i32 coeffs[height*width*(order+1)] ([c0..cn] per pixel)

use crate::engine::{CoefSet, Coefs, Render};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

const HEADER_BYTES: usize = 4 + 8 * 4;

/// What a file's render was searched for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Header {
    pub width: usize,
    pub height: usize,
    pub order: usize,
    pub coefs: Coefs,
    pub legacy: bool,
}

impl Header {
    fn data_bytes(&self) -> u64 {
        let (w, h, t) = (self.width as u64, self.height as u64, self.order as u64 + 1);
        8 * (w + h + w * h) + 4 * w * h * t
    }
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

/// Writes `r`, searched with `order` and `coefs`, to `path`.
pub fn write(path: impl AsRef<Path>, r: &Render, order: usize, coefs: &Coefs, legacy: bool) -> io::Result<()> {
    let mut f = BufWriter::new(File::create(path)?);
    f.write_all(b"ALGF")?;
    for v in [
        2u32,
        r.width as u32,
        r.height as u32,
        order as u32,
        coefs.max as u32,
        1,
        CoefSet::ALL.iter().position(|&c| c == coefs.set).unwrap() as u32,
        legacy as u32 | (!coefs.zero as u32) << 1 | (coefs.zero_root as u32) << 2,
    ] {
        f.write_all(&v.to_le_bytes())?;
    }
    for v in r.re_axis.iter().chain(&r.im_axis).chain(&r.residual) {
        f.write_all(&v.to_le_bytes())?;
    }
    for v in &r.coeffs {
        f.write_all(&v.to_le_bytes())?;
    }
    f.flush()
}

fn parse_header(b: &[u8]) -> io::Result<Header> {
    if b.len() < HEADER_BYTES || &b[..4] != b"ALGF" {
        return Err(bad("not an .algf file"));
    }
    let u = |i: usize| u32::from_le_bytes(b[4 + 4 * i..8 + 4 * i].try_into().unwrap());
    if u(0) != 2 {
        return Err(bad("unknown .algf version"));
    }
    let set = *CoefSet::ALL.get(u(6) as usize).ok_or_else(|| bad("unknown coefficient set"))?;
    let flags = u(7);
    let coefs = Coefs { set, max: u(4) as i32, zero: flags & 2 == 0, zero_root: flags & 4 != 0 };
    let h = Header { width: u(1) as usize, height: u(2) as usize, order: u(3) as usize, coefs, legacy: flags & 1 != 0 };
    if h.width == 0 || h.height == 0 || h.order == 0 || h.order > 16 || coefs.max < 1 {
        return Err(bad("damaged .algf header"));
    }
    Ok(h)
}

/// Just what the file at `path` was searched for.
pub fn read_header(path: impl AsRef<Path>) -> io::Result<Header> {
    let mut b = [0u8; HEADER_BYTES];
    File::open(path)?.read_exact(&mut b)?;
    parse_header(&b)
}

/// The render in the file at `path`, and what it was searched for.
pub fn read(path: impl AsRef<Path>) -> io::Result<(Header, Render)> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut f = BufReader::new(file);
    let mut b = [0u8; HEADER_BYTES];
    f.read_exact(&mut b)?;
    let h = parse_header(&b)?;
    if size != HEADER_BYTES as u64 + h.data_bytes() {
        return Err(bad("the .algf file is the wrong size (cut short?)"));
    }
    let (w, ht, t) = (h.width, h.height, h.order + 1);
    let mut f64s = |n: usize| -> io::Result<Vec<f64>> {
        let mut raw = vec![0u8; 8 * n];
        f.read_exact(&mut raw)?;
        Ok(raw.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect())
    };
    let (re_axis, im_axis, residual) = (f64s(w)?, f64s(ht)?, f64s(w * ht)?);
    let mut raw = vec![0u8; 4 * w * ht * t];
    f.read_exact(&mut raw)?;
    let coeffs = raw.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect();
    Ok((h, Render { width: w, height: ht, nterms: t, re_axis, im_axis, residual, coeffs }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{render, Plan, Region};

    /// A render reads back exactly, with its search settings.
    #[test]
    fn round_trip() {
        let coefs = Coefs { set: CoefSet::Primes, max: 7, zero: false, zero_root: true };
        let plan = Plan::new(3, &coefs, false).unwrap();
        let r = render(&plan, &Region { re: (0.1, 0.5), im: (0.2, 0.45), width: 24 }, 2, &Default::default(), &Default::default()).unwrap();
        let path = std::env::temp_dir().join("algfrac_round_trip.algf");
        write(&path, &r, 3, &coefs, false).unwrap();
        let header = read_header(&path).unwrap();
        let (h, back) = read(&path).unwrap();
        assert_eq!(h, header);
        assert_eq!(h, Header { width: r.width, height: r.height, order: 3, coefs, legacy: false });
        assert_eq!((back.re_axis, back.im_axis, back.residual, back.coeffs), (r.re_axis, r.im_axis, r.residual, r.coeffs));
        // a file cut short is refused
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 4]).unwrap();
        assert!(read(&path).is_err());
        let _ = std::fs::remove_file(path);
    }
}
