//! Baseline JPEG encoder used for MJPEG frames.
//!
//! Written from the JPEG specification (ITU T.81, Annex K tables) for this
//! project. It produces exactly the marker layout classic media players
//! expect from their MJPEG AVI files:
//!
//! `SOI, APP0 "AVI1", DQT(0), DQT(1), SOF0, DHT x4, SOS, <scan>, EOI`
//!
//! with YCbCr 4:2:0 sampling and the standard Huffman tables. The header is
//! 623 bytes long and identical for every frame of a given size, so callers can
//! fetch it once with [`JpegEncoder::header`] and then append scans.

/// Number of bytes of the fixed frame header (SOI up to and including SOS).
pub const HEADER_LEN: usize = 623;

/// Zig-zag position -> natural (row-major) index.
pub const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27,
    20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58,
    59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Annex K.1 luminance quantisation table, natural order.
const STD_LUMA_Q: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69,
    56, 14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104,
    113, 92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// Annex K.1 chrominance quantisation table, natural order.
const STD_CHROMA_Q: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99,
    99, 47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_CHROMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61,
    0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52,
    0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25,
    0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45,
    0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64,
    0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83,
    0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99,
    0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
    0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3,
    0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8,
    0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
];

const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61,
    0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33,
    0x52, 0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18,
    0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44,
    0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63,
    0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a,
    0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97,
    0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4,
    0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca,
    0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7,
    0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
];

/// AAN scale factors used to fold the DCT normalisation into quantisation.
const AAN: [f32; 8] = [
    1.0,
    1.387_039_8,
    1.306_563,
    1.175_875_6,
    1.0,
    0.785_694_96,
    0.541_196_1,
    0.275_899_38,
];

/// Scale a base table with the IJG quality formula (1..=100, 75 = classic players).
pub fn scaled_table(base: &[u16; 64], quality: u8) -> [u8; 64] {
    let q = quality.clamp(1, 100) as u32;
    let scale = if q < 50 { 5000 / q } else { 200 - q * 2 };
    let mut out = [0u8; 64];
    for (o, &b) in out.iter_mut().zip(base.iter()) {
        let v = (b as u32 * scale + 50) / 100;
        *o = v.clamp(1, 255) as u8;
    }
    out
}

/// Luma and chroma quantisation tables (natural order) for a quality value.
pub fn quality_tables(quality: u8) -> ([u8; 64], [u8; 64]) {
    (scaled_table(&STD_LUMA_Q, quality), scaled_table(&STD_CHROMA_Q, quality))
}

#[derive(Clone)]
struct HuffTable {
    code: [u16; 256],
    size: [u8; 256],
}

impl HuffTable {
    fn new(bits: &[u8; 16], vals: &[u8]) -> Self {
        let mut code = [0u16; 256];
        let mut size = [0u8; 256];
        let mut c: u16 = 0;
        let mut k = 0usize;
        for (len_minus_one, &count) in bits.iter().enumerate() {
            for _ in 0..count {
                let sym = vals[k] as usize;
                code[sym] = c;
                size[sym] = (len_minus_one + 1) as u8;
                c += 1;
                k += 1;
            }
            c <<= 1;
        }
        HuffTable { code, size }
    }
}

struct BitWriter<'a> {
    out: &'a mut Vec<u8>,
    acc: u32,
    nbits: u32,
}

impl<'a> BitWriter<'a> {
    fn new(out: &'a mut Vec<u8>) -> Self {
        BitWriter { out, acc: 0, nbits: 0 }
    }

    #[inline]
    fn put(&mut self, value: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.acc = (self.acc << len) | (value & ((1u32 << len) - 1));
        self.nbits += len;
        while self.nbits >= 8 {
            let byte = ((self.acc >> (self.nbits - 8)) & 0xff) as u8;
            self.out.push(byte);
            if byte == 0xff {
                self.out.push(0x00);
            }
            self.nbits -= 8;
        }
        self.acc &= (1u32 << self.nbits).wrapping_sub(1);
    }

    fn flush(&mut self) {
        if self.nbits > 0 {
            let pad = 8 - self.nbits;
            self.put((1u32 << pad) - 1, pad);
        }
    }
}

/// A reusable encoder for frames of one fixed size.
pub struct JpegEncoder {
    width: usize,
    height: usize,
    mcu_w: usize,
    mcu_h: usize,
    header: Vec<u8>,
    luma_div: [f32; 64],
    chroma_div: [f32; 64],
    dc_luma: HuffTable,
    ac_luma: HuffTable,
    dc_chroma: HuffTable,
    ac_chroma: HuffTable,
    y: Vec<u8>,
    cb: Vec<u8>,
    cr: Vec<u8>,
}

impl JpegEncoder {
    /// Create an encoder for `width` x `height` RGB frames.
    pub fn new(width: u32, height: u32, quality: u8) -> Self {
        assert!(width > 0 && height > 0 && width <= 65535 && height <= 65535);
        let (lq, cq) = quality_tables(quality);
        let width = width as usize;
        let height = height as usize;
        let mcu_w = width.div_ceil(16);
        let mcu_h = height.div_ceil(16);
        let pw = mcu_w * 16;
        let ph = mcu_h * 16;
        let mut enc = JpegEncoder {
            width,
            height,
            mcu_w,
            mcu_h,
            header: Vec::with_capacity(HEADER_LEN),
            luma_div: divisors(&lq),
            chroma_div: divisors(&cq),
            dc_luma: HuffTable::new(&DC_LUMA_BITS, &DC_LUMA_VALS),
            ac_luma: HuffTable::new(&AC_LUMA_BITS, &AC_LUMA_VALS),
            dc_chroma: HuffTable::new(&DC_CHROMA_BITS, &DC_CHROMA_VALS),
            ac_chroma: HuffTable::new(&AC_CHROMA_BITS, &AC_CHROMA_VALS),
            y: vec![0; pw * ph],
            cb: vec![0; (pw / 2) * (ph / 2)],
            cr: vec![0; (pw / 2) * (ph / 2)],
        };
        enc.header = build_header(width as u16, height as u16, &lq, &cq);
        debug_assert_eq!(enc.header.len(), HEADER_LEN);
        enc
    }

    pub fn width(&self) -> u32 {
        self.width as u32
    }

    pub fn height(&self) -> u32 {
        self.height as u32
    }

    /// The 623-byte frame header (SOI .. SOS).
    pub fn header(&self) -> &[u8] {
        &self.header
    }

    /// Encode a complete JPEG (header + scan + EOI) into `out`.
    pub fn encode(&mut self, rgb: &[u8], out: &mut Vec<u8>) {
        out.extend_from_slice(&self.header);
        self.encode_scan(rgb, out);
    }

    /// Encode only the entropy-coded scan followed by EOI.
    ///
    /// `rgb` must hold `width * height * 3` bytes, top-down rows.
    pub fn encode_scan(&mut self, rgb: &[u8], out: &mut Vec<u8>) {
        assert_eq!(rgb.len(), self.width * self.height * 3, "frame size mismatch");
        self.convert(rgb);
        let pw = self.mcu_w * 16;
        let cw = pw / 2;
        let mut prev = [0i32; 3];
        let mut block = [0f32; 64];
        let mut bw = BitWriter::new(out);
        for my in 0..self.mcu_h {
            for mx in 0..self.mcu_w {
                for (by, bx) in [(0, 0), (0, 8), (8, 0), (8, 8)] {
                    load_block(&self.y, pw, mx * 16 + bx, my * 16 + by, &mut block);
                    prev[0] = encode_block(
                        &mut bw,
                        &mut block,
                        &self.luma_div,
                        prev[0],
                        &self.dc_luma,
                        &self.ac_luma,
                    );
                }
                load_block(&self.cb, cw, mx * 8, my * 8, &mut block);
                prev[1] = encode_block(
                    &mut bw,
                    &mut block,
                    &self.chroma_div,
                    prev[1],
                    &self.dc_chroma,
                    &self.ac_chroma,
                );
                load_block(&self.cr, cw, mx * 8, my * 8, &mut block);
                prev[2] = encode_block(
                    &mut bw,
                    &mut block,
                    &self.chroma_div,
                    prev[2],
                    &self.dc_chroma,
                    &self.ac_chroma,
                );
            }
        }
        bw.flush();
        out.extend_from_slice(&[0xff, 0xd9]);
    }

    /// RGB -> JFIF YCbCr with edge replication to whole MCUs and 2x2 chroma averaging.
    fn convert(&mut self, rgb: &[u8]) {
        let pw = self.mcu_w * 16;
        let ph = self.mcu_h * 16;
        let cw = pw / 2;
        let ch = ph / 2;
        let (w, h) = (self.width, self.height);
        for y in 0..ph {
            let sy = y.min(h - 1);
            let row = &rgb[sy * w * 3..(sy + 1) * w * 3];
            let dst = &mut self.y[y * pw..(y + 1) * pw];
            for (x, d) in dst.iter_mut().enumerate() {
                let sx = x.min(w - 1) * 3;
                let (r, g, b) = (row[sx] as i32, row[sx + 1] as i32, row[sx + 2] as i32);
                *d = ((19595 * r + 38470 * g + 7471 * b + 32768) >> 16) as u8;
            }
        }
        for cy in 0..ch {
            for cx in 0..cw {
                let mut sum_cb = 0i32;
                let mut sum_cr = 0i32;
                for dy in 0..2 {
                    let sy = (cy * 2 + dy).min(h - 1);
                    for dx in 0..2 {
                        let sx = (cx * 2 + dx).min(w - 1);
                        let p = (sy * w + sx) * 3;
                        let (r, g, b) = (rgb[p] as i32, rgb[p + 1] as i32, rgb[p + 2] as i32);
                        sum_cb += -11059 * r - 21709 * g + 32768 * b;
                        sum_cr += 32768 * r - 27439 * g - 5329 * b;
                    }
                }
                let cb = ((sum_cb + (128 << 18) + (1 << 17)) >> 18).clamp(0, 255);
                let cr = ((sum_cr + (128 << 18) + (1 << 17)) >> 18).clamp(0, 255);
                self.cb[cy * cw + cx] = cb as u8;
                self.cr[cy * cw + cx] = cr as u8;
            }
        }
    }
}

fn divisors(q: &[u8; 64]) -> [f32; 64] {
    let mut d = [0f32; 64];
    for row in 0..8 {
        for col in 0..8 {
            let i = row * 8 + col;
            d[i] = 1.0 / (q[i] as f32 * AAN[row] * AAN[col] * 8.0);
        }
    }
    d
}

fn build_header(width: u16, height: u16, lq: &[u8; 64], cq: &[u8; 64]) -> Vec<u8> {
    let mut h = Vec::with_capacity(HEADER_LEN);
    h.extend_from_slice(&[0xff, 0xd8]);
    // APP0 "AVI1": 14 bytes of payload, identifier then zeroed polarity/field sizes.
    h.extend_from_slice(&[0xff, 0xe0, 0x00, 0x10, b'A', b'V', b'I', b'1']);
    h.extend_from_slice(&[0u8; 10]);
    for (id, table) in [(0u8, lq), (1u8, cq)] {
        h.extend_from_slice(&[0xff, 0xdb, 0x00, 0x43, id]);
        for &zz in ZIGZAG.iter() {
            h.push(table[zz]);
        }
    }
    h.extend_from_slice(&[0xff, 0xc0, 0x00, 0x11, 0x08]);
    h.extend_from_slice(&height.to_be_bytes());
    h.extend_from_slice(&width.to_be_bytes());
    h.extend_from_slice(&[0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]);
    let tables: [(u8, &[u8; 16], &[u8]); 4] = [
        (0x00, &DC_LUMA_BITS, &DC_LUMA_VALS),
        (0x10, &AC_LUMA_BITS, &AC_LUMA_VALS),
        (0x01, &DC_CHROMA_BITS, &DC_CHROMA_VALS),
        (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALS),
    ];
    for (class_id, bits, vals) in tables {
        let len = 2 + 1 + 16 + vals.len();
        h.extend_from_slice(&[0xff, 0xc4]);
        h.extend_from_slice(&(len as u16).to_be_bytes());
        h.push(class_id);
        h.extend_from_slice(bits);
        h.extend_from_slice(vals);
    }
    h.extend_from_slice(&[
        0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00,
    ]);
    h
}

#[inline]
fn load_block(plane: &[u8], stride: usize, x0: usize, y0: usize, block: &mut [f32; 64]) {
    for r in 0..8 {
        let base = (y0 + r) * stride + x0;
        let src = &plane[base..base + 8];
        for c in 0..8 {
            block[r * 8 + c] = src[c] as f32 - 128.0;
        }
    }
}

/// Forward DCT (Arai, Agui, Nakajima), output scaled by 8 * AAN[u] * AAN[v].
#[inline]
fn fdct(d: &mut [f32; 64]) {
    for pass in 0..2 {
        for i in 0..8 {
            let idx = |k: usize| if pass == 0 { i * 8 + k } else { k * 8 + i };
            let (i0, i1, i2, i3, i4, i5, i6, i7) =
                (idx(0), idx(1), idx(2), idx(3), idx(4), idx(5), idx(6), idx(7));
            let tmp0 = d[i0] + d[i7];
            let tmp7 = d[i0] - d[i7];
            let tmp1 = d[i1] + d[i6];
            let tmp6 = d[i1] - d[i6];
            let tmp2 = d[i2] + d[i5];
            let tmp5 = d[i2] - d[i5];
            let tmp3 = d[i3] + d[i4];
            let tmp4 = d[i3] - d[i4];

            let tmp10 = tmp0 + tmp3;
            let tmp13 = tmp0 - tmp3;
            let tmp11 = tmp1 + tmp2;
            let tmp12 = tmp1 - tmp2;
            d[i0] = tmp10 + tmp11;
            d[i4] = tmp10 - tmp11;
            let z1 = (tmp12 + tmp13) * 0.707_106_77;
            d[i2] = tmp13 + z1;
            d[i6] = tmp13 - z1;

            let tmp10 = tmp4 + tmp5;
            let tmp11 = tmp5 + tmp6;
            let tmp12 = tmp6 + tmp7;
            let z5 = (tmp10 - tmp12) * 0.382_683_43;
            let z2 = 0.541_196_1 * tmp10 + z5;
            let z4 = 1.306_563 * tmp12 + z5;
            let z3 = tmp11 * 0.707_106_77;
            let z11 = tmp7 + z3;
            let z13 = tmp7 - z3;
            d[i5] = z13 + z2;
            d[i3] = z13 - z2;
            d[i1] = z11 + z4;
            d[i7] = z11 - z4;
        }
    }
}

#[inline]
fn magnitude(v: i32) -> (u32, u32) {
    let a = v.unsigned_abs();
    let n = 32 - a.leading_zeros();
    let bits = if v < 0 { (v - 1) as u32 } else { v as u32 };
    (n, bits)
}

#[inline]
fn encode_block(
    bw: &mut BitWriter,
    block: &mut [f32; 64],
    div: &[f32; 64],
    prev_dc: i32,
    dc: &HuffTable,
    ac: &HuffTable,
) -> i32 {
    fdct(block);
    let mut q = [0i32; 64];
    for (k, &zz) in ZIGZAG.iter().enumerate() {
        q[k] = (block[zz] * div[zz]).round() as i32;
    }
    let diff = q[0] - prev_dc;
    let (n, bits) = magnitude(diff);
    bw.put(dc.code[n as usize] as u32, dc.size[n as usize] as u32);
    bw.put(bits, n);

    let mut run = 0u32;
    for &v in q.iter().skip(1) {
        if v == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            bw.put(ac.code[0xf0] as u32, ac.size[0xf0] as u32);
            run -= 16;
        }
        let (n, bits) = magnitude(v);
        let sym = ((run << 4) | n) as usize;
        bw.put(ac.code[sym] as u32, ac.size[sym] as u32);
        bw.put(bits, n);
        run = 0;
    }
    if run > 0 {
        bw.put(ac.code[0x00] as u32, ac.size[0x00] as u32);
    }
    q[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_75_matches_ijg_half_tables() {
        let (l, c) = quality_tables(75);
        assert_eq!(l[0], 8);
        assert_eq!(l[1], 6);
        assert_eq!(c[0], 9);
        assert_eq!(c[63], 50);
    }

    #[test]
    fn header_layout() {
        let enc = JpegEncoder::new(160, 120, 75);
        let h = enc.header();
        assert_eq!(h.len(), HEADER_LEN);
        assert_eq!(&h[0..2], &[0xff, 0xd8]);
        assert_eq!(&h[6..10], b"AVI1");
        // SOF0 dimensions
        let sof = h.windows(2).position(|w| w == [0xff, 0xc0]).unwrap();
        assert_eq!(&h[sof + 5..sof + 9], &[0x00, 120, 0x00, 160]);
        assert_eq!(&h[h.len() - 14..h.len() - 12], &[0xff, 0xda]);
    }

    #[test]
    fn encodes_and_terminates() {
        let mut enc = JpegEncoder::new(33, 17, 75);
        let mut rgb = vec![0u8; 33 * 17 * 3];
        for (i, p) in rgb.iter_mut().enumerate() {
            *p = (i * 7 % 251) as u8;
        }
        let mut out = Vec::new();
        enc.encode(&rgb, &mut out);
        assert_eq!(&out[out.len() - 2..], &[0xff, 0xd9]);
        assert!(out.len() > HEADER_LEN + 10);
    }
}
