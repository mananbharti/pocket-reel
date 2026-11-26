//! AVI writer for the classic media-player format (MJPEG video + IMA ADPCM audio).
//!
//! The header layout and every field value follow the behaviour documented in
//! `docs/FINDINGS.md` section 3.6, including the non-standard fields old player
//! firmware reads (`avih.dwReserved[0]` = fps * 1024, `dwReserved[3]` = duration
//! as `0x00HHMMSS`) and the original's stream-length quirks. Chunks are aligned to
//! 4 bytes and an `idx1` index is appended.

use std::io::{self, Seek, SeekFrom, Write};

use crate::adpcm::mono_block_align;
use crate::jpeg::HEADER_LEN as JPEG_HEADER_LEN;

/// Size of the fixed RIFF/hdrl/movi header in bytes.
pub const HEADER_LEN: usize = 0x148;
/// Sample rate used by classic players.
pub const CLASSIC_SAMPLE_RATE: u32 = 22050;

const AVIF_HASINDEX: u32 = 0x10;
const AVIF_WASCAPTUREFILE: u32 = 0x1_0000;
const AVIIF_KEYFRAME: u32 = 0x10;

/// Samples per ADPCM block (and therefore frame rate) for a requested fps.
///
/// The original converter uses 1017 samples (21.68 fps) when the requested
/// rate is 18 fps or more, otherwise 1529 samples (14.42 fps).
pub fn samples_per_block_for_fps(requested_fps: f64) -> u32 {
    if requested_fps >= 18.0 {
        1017
    } else {
        1529
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassicParams {
    pub width: u32,
    pub height: u32,
    pub samples_per_block: u32,
}

impl ClassicParams {
    pub fn new(width: u32, height: u32, samples_per_block: u32) -> Self {
        ClassicParams { width, height, samples_per_block }
    }

    pub fn block_align(&self) -> u32 {
        mono_block_align(self.samples_per_block as usize) as u32
    }

    /// Microseconds per frame, integer division as in the original encoder.
    pub fn us_per_frame(&self) -> u32 {
        (self.samples_per_block as u64 * 1_000_000 / CLASSIC_SAMPLE_RATE as u64) as u32
    }

    /// Exact frame rate as a rational (numerator, denominator).
    pub fn fps_rational(&self) -> (u32, u32) {
        (CLASSIC_SAMPLE_RATE, self.samples_per_block)
    }

    pub fn fps(&self) -> f64 {
        CLASSIC_SAMPLE_RATE as f64 / self.samples_per_block as f64
    }

    pub fn avg_bytes_per_sec(&self) -> u32 {
        self.block_align() * CLASSIC_SAMPLE_RATE / self.samples_per_block
    }

    /// Bytes added to the file per frame excluding the JPEG scan itself.
    pub fn fixed_bytes_per_frame(&self) -> u64 {
        // 00dc chunk header + JPEG header, 01wb chunk header + block, 2 index entries.
        (8 + JPEG_HEADER_LEN as u64) + (8 + self.block_align() as u64) + 32
    }
}

/// Pack a duration in seconds as `0x00HHMMSS`.
pub fn pack_hhmmss(total_secs: u32) -> u32 {
    let h = (total_secs / 3600).min(255);
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    (h << 16) | (m << 8) | s
}

#[derive(Debug, Clone, Copy)]
struct IndexEntry {
    video: bool,
    offset: u32,
    size: u32,
}

/// Streaming writer. Call [`ClassicAviWriter::write_frame`] once per frame and
/// [`ClassicAviWriter::finish`] at the end.
pub struct ClassicAviWriter<W: Write + Seek> {
    out: W,
    params: ClassicParams,
    frames: u32,
    movi_size: u64,
    index: Vec<IndexEntry>,
}

impl<W: Write + Seek> ClassicAviWriter<W> {
    pub fn new(mut out: W, params: ClassicParams) -> io::Result<Self> {
        let header = build_header(&params, 0, 4, 0, 0);
        out.write_all(&header)?;
        Ok(ClassicAviWriter { out, params, frames: 0, movi_size: 4, index: Vec::new() })
    }

    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Bytes written so far (approximate final size minus the index).
    pub fn bytes_written(&self) -> u64 {
        HEADER_LEN as u64 + self.movi_size - 4
    }

    /// Write one `00dc` + `01wb` pair. `jpeg_header` is the 623-byte frame
    /// header, `scan` the entropy-coded data ending with EOI.
    pub fn write_frame(&mut self, jpeg_header: &[u8], scan: &[u8], audio_block: &[u8]) -> io::Result<()> {
        debug_assert_eq!(audio_block.len() as u32, self.params.block_align());
        let size = (jpeg_header.len() + scan.len()) as u32;
        let pad = (4 - ((8 + size) % 4)) % 4;
        let offset = self.movi_size as u32;
        self.out.write_all(b"00dc")?;
        self.out.write_all(&size.to_le_bytes())?;
        self.out.write_all(jpeg_header)?;
        self.out.write_all(scan)?;
        self.out.write_all(&[0u8; 3][..pad as usize])?;
        self.index.push(IndexEntry { video: true, offset, size: size + pad });
        self.movi_size += (8 + size + pad) as u64;

        let offset = self.movi_size as u32;
        let asize = audio_block.len() as u32;
        self.out.write_all(b"01wb")?;
        self.out.write_all(&asize.to_le_bytes())?;
        self.out.write_all(audio_block)?;
        self.index.push(IndexEntry { video: false, offset, size: asize });
        self.movi_size += (8 + asize) as u64;
        self.frames += 1;
        Ok(())
    }

    /// Write the index and the final header. `duration_secs` is stored as HHMMSS.
    pub fn finish(mut self, duration_secs: u32) -> io::Result<W> {
        let idx_size = self.index.len() as u32 * 16;
        let mut idx = Vec::with_capacity(8 + idx_size as usize);
        idx.extend_from_slice(b"idx1");
        idx.extend_from_slice(&idx_size.to_le_bytes());
        for e in &self.index {
            idx.extend_from_slice(if e.video { b"00dc" } else { b"01wb" });
            idx.extend_from_slice(&AVIIF_KEYFRAME.to_le_bytes());
            idx.extend_from_slice(&e.offset.to_le_bytes());
            idx.extend_from_slice(&e.size.to_le_bytes());
        }
        self.out.write_all(&idx)?;
        let header =
            build_header(&self.params, self.frames, self.movi_size as u32, idx_size, pack_hhmmss(duration_secs));
        self.out.seek(SeekFrom::Start(0))?;
        self.out.write_all(&header)?;
        self.out.seek(SeekFrom::End(0))?;
        self.out.flush()?;
        Ok(self.out)
    }
}

fn put(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}

fn put16(b: &mut Vec<u8>, v: u16) {
    b.extend_from_slice(&v.to_le_bytes());
}

/// Build the 0x148-byte header.
pub fn build_header(p: &ClassicParams, frames: u32, movi_size: u32, idx_size: u32, hhmmss: u32) -> Vec<u8> {
    let us = p.us_per_frame();
    let block_align = p.block_align();
    let spb = p.samples_per_block;
    let audio_unit = (spb >> 2) + 4;
    let mut h = Vec::with_capacity(HEADER_LEN);
    h.extend_from_slice(b"RIFF");
    put(&mut h, movi_size.wrapping_add(idx_size).wrapping_add(0x144));
    h.extend_from_slice(b"AVI LIST");
    put(&mut h, 0x128);
    h.extend_from_slice(b"hdrlavih");
    put(&mut h, 0x38);
    put(&mut h, us); // dwMicroSecPerFrame
    put(&mut h, 0x40000); // dwMaxBytesPerSec
    put(&mut h, 0); // dwPaddingGranularity
    put(&mut h, AVIF_HASINDEX | AVIF_WASCAPTUREFILE);
    put(&mut h, frames); // dwTotalFrames
    put(&mut h, 0); // dwInitialFrames
    put(&mut h, 2); // dwStreams
    put(&mut h, 0x40000); // dwSuggestedBufferSize
    put(&mut h, p.width);
    put(&mut h, p.height);
    put(&mut h, 22_579_200 / spb); // dwReserved[0]: fps * 1024
    put(&mut h, 1); // dwReserved[1]
    put(&mut h, 0); // dwReserved[2]
    put(&mut h, hhmmss); // dwReserved[3]

    // Video stream list
    h.extend_from_slice(b"LIST");
    put(&mut h, 0x74);
    h.extend_from_slice(b"strlstrh");
    put(&mut h, 0x38);
    h.extend_from_slice(b"vidsMJPG");
    put(&mut h, 0); // dwFlags
    put16(&mut h, 0); // wPriority
    put16(&mut h, 0); // wLanguage
    put(&mut h, 0); // dwInitialFrames
    put(&mut h, us); // dwScale
    put(&mut h, 1_000_000); // dwRate
    put(&mut h, 0); // dwStart
    put(&mut h, 2); // dwLength (constant in the original)
    put(&mut h, 0); // dwSuggestedBufferSize
    put(&mut h, 0); // dwQuality
    put(&mut h, 0); // dwSampleSize
    put16(&mut h, 0);
    put16(&mut h, 0);
    put16(&mut h, p.width as u16);
    put16(&mut h, p.height as u16);
    h.extend_from_slice(b"strf");
    put(&mut h, 0x28);
    put(&mut h, 40);
    put(&mut h, p.width);
    put(&mut h, p.height);
    put16(&mut h, 1); // planes
    put16(&mut h, 24); // bit count
    h.extend_from_slice(b"MJPG");
    put(&mut h, p.width * p.height * 3);
    put(&mut h, 0);
    put(&mut h, 0);
    put(&mut h, 0);
    put(&mut h, 0);

    // Audio stream list
    h.extend_from_slice(b"LIST");
    put(&mut h, 0x60);
    h.extend_from_slice(b"strlstrh");
    put(&mut h, 0x38);
    h.extend_from_slice(b"auds");
    put(&mut h, 0); // fccHandler
    put(&mut h, 0); // dwFlags
    put16(&mut h, 0);
    put16(&mut h, 0);
    put(&mut h, 0); // dwInitialFrames
    put(&mut h, block_align); // dwScale
    put(&mut h, p.avg_bytes_per_sec()); // dwRate
    put(&mut h, 0); // dwStart
    put(&mut h, audio_unit.wrapping_mul(frames)); // dwLength (original's formula)
    put(&mut h, audio_unit); // dwSuggestedBufferSize
    put(&mut h, 0); // dwQuality
    put(&mut h, block_align); // dwSampleSize
    put(&mut h, 0);
    put(&mut h, 0);
    h.extend_from_slice(b"strf");
    put(&mut h, 0x14);
    put16(&mut h, 0x0011); // IMA ADPCM
    put16(&mut h, 1); // channels
    put(&mut h, CLASSIC_SAMPLE_RATE);
    put(&mut h, p.avg_bytes_per_sec());
    put16(&mut h, block_align as u16);
    put16(&mut h, 4); // bits per sample
    put16(&mut h, 2); // cbSize
    put16(&mut h, spb as u16);

    h.extend_from_slice(b"LIST");
    put(&mut h, movi_size);
    h.extend_from_slice(b"movi");
    debug_assert_eq!(h.len(), HEADER_LEN);
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn derived_constants_match_findings() {
        let low = ClassicParams::new(160, 120, 1529);
        assert_eq!(low.block_align(), 768);
        assert_eq!(low.us_per_frame(), 69342);
        assert_eq!(low.avg_bytes_per_sec(), 11075);
        let high = ClassicParams::new(160, 120, 1017);
        assert_eq!(high.block_align(), 512);
        assert_eq!(high.us_per_frame(), 46122);
        assert_eq!(high.avg_bytes_per_sec(), 11100);
        assert_eq!(samples_per_block_for_fps(22.0), 1017);
        assert_eq!(samples_per_block_for_fps(14.0), 1529);
    }

    #[test]
    fn hhmmss_packing() {
        assert_eq!(pack_hhmmss(3725), 0x0001_0205);
    }

    #[test]
    fn writes_aligned_chunks_and_index() {
        let p = ClassicParams::new(16, 16, 1017);
        let mut w = ClassicAviWriter::new(Cursor::new(Vec::new()), p).unwrap();
        let hdr = vec![0u8; JPEG_HEADER_LEN];
        let block = vec![0u8; 512];
        w.write_frame(&hdr, &[1, 2, 3, 0xff, 0xd9], &block).unwrap();
        w.write_frame(&hdr, &[1, 0xff, 0xd9], &block).unwrap();
        let data = w.finish(7).unwrap().into_inner();
        assert_eq!(&data[0..4], b"RIFF");
        let riff = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff + 8, data.len());
        // second chunk must start 4-byte aligned
        let first_size = u32::from_le_bytes(data[HEADER_LEN + 4..HEADER_LEN + 8].try_into().unwrap());
        assert_eq!(first_size, (JPEG_HEADER_LEN + 5) as u32);
        let next = HEADER_LEN + 8 + first_size as usize;
        let next = next + (4 - next % 4) % 4;
        assert_eq!(&data[next..next + 4], b"01wb");
        assert_eq!(&data[data.len() - 64 - 8..data.len() - 64 - 4], b"idx1");
        // dwReserved[3]
        assert_eq!(u32::from_le_bytes(data[32 + 52..32 + 56].try_into().unwrap()), 7);
    }
}
