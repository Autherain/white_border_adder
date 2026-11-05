//! The shot date from the head of a file: a 25 MB raw costs one 256 kB read.
//! JPEG carries it in an APP1 segment; RW2, DNG and TIFF are TIFF containers
//! from byte 0. RW2 has its own magic number instead of 42, which is why no
//! EXIF crate is used: kamadak-exif rejects it.
//!
//! Every function answers `Option`: `None` is "no date here", whatever the
//! reason, and `?` on an `Option` returns `None` at once, the way Go code
//! returns `(0, false)`. The bytes come from a file we do not control, so
//! every read goes through `get(..)`, which is `None` past the end, never a
//! panic.

use std::io::Read;

use jiff::civil::DateTime;
use vfs::VfsPath;

const TAG_EXIF_IFD: u16 = 0x8769;
const TAG_DATE_TIME_ORIGINAL: u16 = 0x9003;
const TAG_DATE_TIME: u16 = 0x0132;
const HEAD_SIZE: u64 = 256 << 10;
const IFD_ENTRY_SIZE: usize = 12;
const TYPE_ASCII: u16 = 2;
const TYPE_LONG: u16 = 4;
const TIME_LEN: usize = 19;
const MAX_IFD_ENTRIES: u16 = 512;
const JPEG_SOI: [u8; 2] = [0xFF, 0xD8];
const JPEG_APP1: u8 = 0xE1;
const JPEG_SOS: u8 = 0xDA;
const JPEG_SEGMENT_START: u8 = 0xFF;

pub fn shot_date(path: &VfsPath) -> Option<DateTime> {
    let head = read_head(path)?;
    if head.starts_with(&JPEG_SOI) {
        let tiff = jpeg_exif(&head)?;
        return tiff_date(tiff);
    }
    tiff_date(&head)
}

fn read_head(path: &VfsPath) -> Option<Vec<u8>> {
    let Ok(file) = path.open_file() else {
        return None;
    };
    let mut head = Vec::new();
    if file.take(HEAD_SIZE).read_to_end(&mut head).is_err() {
        return None;
    }
    Some(head)
}

fn jpeg_exif(b: &[u8]) -> Option<&[u8]> {
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != JPEG_SEGMENT_START {
            return None;
        }
        let marker = b[i + 1];
        let size = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
        if marker == JPEG_SOS || size < 2 {
            return None;
        }
        let end = usize::min(i + 2 + size, b.len());
        if marker == JPEG_APP1 && b[i + 4..end].starts_with(b"Exif\0\0") {
            return b.get(i + 10..end);
        }
        i += 2 + size;
    }
    None
}

/// TIFF says in its first two bytes which way round its numbers are written.
#[derive(Clone, Copy)]
enum Order {
    Little,
    Big,
}

impl Order {
    fn u16(self, b: &[u8], at: usize) -> Option<u16> {
        let bytes = b.get(at..at + 2)?;
        let pair = [bytes[0], bytes[1]];
        match self {
            Self::Little => Some(u16::from_le_bytes(pair)),
            Self::Big => Some(u16::from_be_bytes(pair)),
        }
    }

    fn u32(self, b: &[u8], at: usize) -> Option<u32> {
        let bytes = b.get(at..at + 4)?;
        let quad = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::Little => Some(u32::from_le_bytes(quad)),
            Self::Big => Some(u32::from_be_bytes(quad)),
        }
    }

    /// A u32 used as a position in the file.
    fn offset(self, b: &[u8], at: usize) -> Option<usize> {
        let value = self.u32(b, at)?;
        usize::try_from(value).ok()
    }
}

fn tiff_date(b: &[u8]) -> Option<DateTime> {
    let order = match b.get(..2)? {
        b"II" => Order::Little,
        b"MM" => Order::Big,
        _ => return None,
    };
    let ifd0 = order.offset(b, 4)?;

    // The camera's own date lives in the EXIF sub-directory; the one in
    // IFD0 is the file's, which an editor may have rewritten.
    if let Some(exif_ifd) = find_tag(b, order, ifd0, TAG_EXIF_IFD)
        && let Some(t) = time_at(b, order, exif_ifd, TAG_DATE_TIME_ORIGINAL)
    {
        return Some(t);
    }
    time_at(b, order, ifd0, TAG_DATE_TIME)
}

fn time_at(b: &[u8], order: Order, ifd: usize, tag: u16) -> Option<DateTime> {
    let off = find_tag(b, order, ifd, tag)?;
    let bytes = b.get(off..off + TIME_LEN)?;
    let Ok(text) = std::str::from_utf8(bytes) else {
        return None;
    };
    let Ok(t) = DateTime::strptime("%Y:%m:%d %H:%M:%S", text) else {
        return None;
    };
    if t.year() <= 1970 {
        return None;
    }
    Some(t)
}

/// The value field of a tag: the offset it holds for an ASCII string or a
/// pointer, which is what both callers want.
fn find_tag(b: &[u8], order: Order, ifd: usize, tag: u16) -> Option<usize> {
    if ifd == 0 {
        return None;
    }
    let count = order.u16(b, ifd)?.min(MAX_IFD_ENTRIES);

    for i in 0..usize::from(count) {
        let e = ifd + 2 + i * IFD_ENTRY_SIZE;
        if order.u16(b, e)? != tag {
            continue;
        }
        let kind = order.u16(b, e + 2)?;
        if kind != TYPE_ASCII && kind != TYPE_LONG {
            return None;
        }
        return order.offset(b, e + 8);
    }
    None
}

#[cfg(test)]
#[path = "exif_test.rs"]
pub(crate) mod tests;
