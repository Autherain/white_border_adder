package sorter

import (
	"bytes"
	"encoding/binary"
	"io"
	"time"

	"github.com/spf13/afero"
)

const (
	tagExifIFD       = 0x8769
	tagDateTimeOrig  = 0x9003
	tagDateTime      = 0x0132
	exifTimeLayout   = "2006:01:02 15:04:05"
	exifHeadSize     = 256 << 10
	ifdEntrySize     = 12
	typeASCII        = 2
	typeLong         = 4
	exifTimeLen      = 19
	maxIFDEntries    = 512
	jpegSOI          = 0xFFD8
	jpegAPP1         = 0xE1
	jpegSOS          = 0xDA
	jpegSegmentStart = 0xFF
)

// shotDate reads the moment the camera recorded, from the head of the file
// only: a 25 MB raw costs one 256 kB read. JPEG carries it in an APP1 segment;
// RW2, DNG and TIFF are TIFF containers from byte 0. RW2 has its own magic
// number instead of 42, which is why this does not use a TIFF library.
func shotDate(fs afero.Fs, path string) (time.Time, bool) {
	f, err := fs.Open(path)
	if err != nil {
		return time.Time{}, false
	}

	defer func() { _ = f.Close() }()

	head := make([]byte, exifHeadSize)

	n, err := io.ReadFull(f, head)
	if err != nil && err != io.ErrUnexpectedEOF && err != io.EOF {
		return time.Time{}, false
	}

	head = head[:n]

	tiff := head
	if len(head) > 2 && binary.BigEndian.Uint16(head) == jpegSOI {
		tiff = jpegExif(head)
	}

	return tiffDate(tiff)
}

func jpegExif(b []byte) []byte {
	for i := 2; i+4 <= len(b); {
		if b[i] != jpegSegmentStart {
			return nil
		}

		marker, size := b[i+1], int(binary.BigEndian.Uint16(b[i+2:]))
		if marker == jpegSOS || size < 2 {
			return nil
		}

		end := min(i+2+size, len(b))
		if marker == jpegAPP1 && bytes.HasPrefix(b[i+4:end], []byte("Exif\x00\x00")) {
			return b[i+10 : end]
		}

		i += 2 + size
	}

	return nil
}

func tiffDate(b []byte) (time.Time, bool) {
	if len(b) < 8 {
		return time.Time{}, false
	}

	var order binary.ByteOrder

	switch string(b[:2]) {
	case "II":
		order = binary.LittleEndian
	case "MM":
		order = binary.BigEndian
	default:
		return time.Time{}, false
	}

	ifd0 := int(order.Uint32(b[4:]))

	exifIFD, _ := findTag(b, order, ifd0, tagExifIFD)
	if t, ok := timeAt(b, order, exifIFD, tagDateTimeOrig); ok {
		return t, true
	}

	return timeAt(b, order, ifd0, tagDateTime)
}

func timeAt(b []byte, order binary.ByteOrder, ifd int, tag uint16) (time.Time, bool) {
	off, ok := findTag(b, order, ifd, tag)
	if !ok || off < 0 || off+exifTimeLen > len(b) {
		return time.Time{}, false
	}

	t, err := time.ParseInLocation(exifTimeLayout, string(b[off:off+exifTimeLen]), time.Local)

	return t, err == nil && t.Year() > 1970
}

// findTag returns the value field of a tag: the offset it holds for an ASCII
// string or a pointer, which is what both callers want. Every read is bounds
// checked since the bytes come from a file we do not control.
func findTag(b []byte, order binary.ByteOrder, ifd int, tag uint16) (int, bool) {
	if ifd <= 0 || ifd+2 > len(b) {
		return 0, false
	}

	count := min(int(order.Uint16(b[ifd:])), maxIFDEntries)

	for i := range count {
		e := ifd + 2 + i*ifdEntrySize
		if e+ifdEntrySize > len(b) {
			return 0, false
		}

		if order.Uint16(b[e:]) != tag {
			continue
		}

		if kind := order.Uint16(b[e+2:]); kind != typeASCII && kind != typeLong {
			return 0, false
		}

		return int(order.Uint32(b[e+8:])), true
	}

	return 0, false
}
