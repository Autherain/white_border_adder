//! The shot date from the head of a file: a 25 MB raw costs one 256 kB read.
//! JPEG carries it in an APP1 segment; RW2, DNG and TIFF are TIFF containers
//! from byte 0. RW2 has its own magic number instead of 42, which is why no
//! EXIF crate is used: kamadak-exif rejects it.
//!
//! Every function answers `Option`: `None` is "no date here", whatever the
//! reason, and `?` on an `Option` returns `None` at once. The bytes come
//! from a file we do not control, so
//! every read goes through `get(..)`, which is `None` past the end, never a
//! panic.

use std::io::Read;

use jiff::civil::DateTime;
use vfs::VfsPath;

const TAG_EXIF_IFD: u16 = 0x8769;
const TAG_DATE_TIME_ORIGINAL: u16 = 0x9003;
const TAG_DATE_TIME: u16 = 0x0132;
const HEAD_SIZE: u64 = 256 << 10;
/// An IFD (a TIFF directory) is a 2-byte entry count, then entries of 12
/// bytes: tag (2), type (2), count (4), value or offset of the value (4).
const IFD_ENTRY_SIZE: usize = 12;
const IFD_ENTRY_TYPE_AT: usize = 2;
const IFD_ENTRY_VALUE_AT: usize = 8;
const TYPE_ASCII: u16 = 2;
const TYPE_LONG: u16 = 4;
/// "2025:08:04 12:30:00".
const TIME_LEN: usize = 19;
/// A corrupt count must not send the search through the whole head.
const MAX_IFD_ENTRIES: u16 = 512;
const JPEG_SOI: [u8; 2] = [0xFF, 0xD8];
const JPEG_APP1: u8 = 0xE1;
const JPEG_SOS: u8 = 0xDA;
const JPEG_SEGMENT_START: u8 = 0xFF;
/// FF, the marker, then the size on two bytes.
const JPEG_SEGMENT_HEADER: usize = 4;
const JPEG_EXIF_MAGIC: &[u8] = b"Exif\0\0";

pub fn shot_date(path: &VfsPath) -> Option<DateTime> {
    let head = read_head(path)?;
    if head.starts_with(&JPEG_SOI) {
        let tiff = jpeg_exif(&head)?;
        return tiff_date(tiff);
    }
    tiff_date(&head)
}

fn read_head(path: &VfsPath) -> Option<Vec<u8>> {
    let file = match path.open_file() {
        Ok(f) => f,
        Err(_) => return None,
    };
    let mut head = Vec::new();
    if file.take(HEAD_SIZE).read_to_end(&mut head).is_err() {
        return None;
    }
    Some(head)
}

/// The TIFF block a JPEG carries. A JPEG is a row of segments, each one
/// `FF <marker> <size> <payload>`; EXIF is the APP1 segment whose payload
/// opens with "Exif\0\0", and the TIFF block is what follows.
fn jpeg_exif(head: &[u8]) -> Option<&[u8]> {
    let mut segment = JPEG_SOI.len();
    while segment + JPEG_SEGMENT_HEADER <= head.len() {
        if head[segment] != JPEG_SEGMENT_START {
            return None;
        }
        let marker = head[segment + 1];
        // The size counts its own two bytes but not FF and the marker.
        let size = usize::from(u16::from_be_bytes([head[segment + 2], head[segment + 3]]));
        // SOS opens the picture itself: no metadata comes after it.
        if marker == JPEG_SOS || size < 2 {
            return None;
        }
        let payload = segment + JPEG_SEGMENT_HEADER;
        let end = usize::min(segment + 2 + size, head.len());
        if marker == JPEG_APP1 && head[payload..end].starts_with(JPEG_EXIF_MAGIC) {
            return head.get(payload + JPEG_EXIF_MAGIC.len()..end);
        }
        segment += 2 + size;
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
    fn u16(self, tiff: &[u8], at: usize) -> Option<u16> {
        let bytes = tiff.get(at..at + 2)?;
        let pair = [bytes[0], bytes[1]];
        match self {
            Self::Little => Some(u16::from_le_bytes(pair)),
            Self::Big => Some(u16::from_be_bytes(pair)),
        }
    }

    fn u32(self, tiff: &[u8], at: usize) -> Option<u32> {
        let bytes = tiff.get(at..at + 4)?;
        let quad = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::Little => Some(u32::from_le_bytes(quad)),
            Self::Big => Some(u32::from_be_bytes(quad)),
        }
    }

    /// A u32 used as a position in the file.
    fn offset(self, tiff: &[u8], at: usize) -> Option<usize> {
        let value = self.u32(tiff, at)?;
        match usize::try_from(value) {
            Ok(off) => Some(off),
            Err(_) => None,
        }
    }
}

/// The date in a TIFF block: byte order on 2 bytes, a magic number on 2,
/// then the offset of the first directory, IFD0.
fn tiff_date(tiff: &[u8]) -> Option<DateTime> {
    let order = match tiff.get(..2)? {
        b"II" => Order::Little,
        b"MM" => Order::Big,
        _ => return None,
    };
    let ifd0 = order.offset(tiff, 4)?;

    // The camera's own date first; the one in IFD0 is the file's, which an
    // editor may have rewritten.
    match camera_date(tiff, order, ifd0) {
        Some(t) => Some(t),
        None => time_at(tiff, order, ifd0, TAG_DATE_TIME),
    }
}

/// The date the camera wrote, kept in the EXIF sub-directory IFD0 points to.
fn camera_date(tiff: &[u8], order: Order, ifd0: usize) -> Option<DateTime> {
    let exif_ifd = find_tag(tiff, order, ifd0, TAG_EXIF_IFD)?;
    time_at(tiff, order, exif_ifd, TAG_DATE_TIME_ORIGINAL)
}

/// The date a tag of this directory points to.
fn time_at(tiff: &[u8], order: Order, ifd: usize, tag: u16) -> Option<DateTime> {
    let off = find_tag(tiff, order, ifd, tag)?;
    let ascii = tiff.get(off..off + TIME_LEN)?;
    let text = match std::str::from_utf8(ascii) {
        Ok(t) => t,
        Err(_) => return None,
    };
    let time = match DateTime::strptime("%Y:%m:%d %H:%M:%S", text) {
        Ok(t) => t,
        Err(_) => return None,
    };
    // A date at or before the epoch is a clock that was never set; no date
    // is better than filing the photo under 1970.
    if time.year() <= 1970 {
        return None;
    }
    Some(time)
}

/// The value field of a tag: the offset it holds for an ASCII string or a
/// pointer, which is what both callers want.
fn find_tag(tiff: &[u8], order: Order, ifd: usize, tag: u16) -> Option<usize> {
    if ifd == 0 {
        return None;
    }
    let count = order.u16(tiff, ifd)?.min(MAX_IFD_ENTRIES);

    for index in 0..usize::from(count) {
        let entry = ifd + 2 + index * IFD_ENTRY_SIZE;
        if order.u16(tiff, entry)? != tag {
            continue;
        }
        let kind = order.u16(tiff, entry + IFD_ENTRY_TYPE_AT)?;
        if kind != TYPE_ASCII && kind != TYPE_LONG {
            return None;
        }
        return order.offset(tiff, entry + IFD_ENTRY_VALUE_AT);
    }
    None
}

#[cfg(test)]
#[path = "exif_test.rs"]
pub(crate) mod tests;
