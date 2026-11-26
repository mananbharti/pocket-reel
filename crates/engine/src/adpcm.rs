//! IMA ADPCM encoder producing Microsoft "IMA ADPCM" (WAVE_FORMAT 0x0011) blocks.
//!
//! Mono block layout: `i16 first sample, u8 step index, u8 0`, then
//! `samples_per_block - 1` 4-bit codes packed two per byte, low nibble first.
//! The encoder keeps its step index across blocks and, for every sample, picks
//! the code that minimises the reconstruction error (the decoder's arithmetic
//! is simulated exactly, so there is no drift).

pub const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408,
    449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630,
    9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

pub const INDEX_TABLE: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// Bytes per mono block for a given number of samples per block.
pub const fn mono_block_align(samples_per_block: usize) -> usize {
    (samples_per_block + 7) / 2
}

#[derive(Debug, Clone, Default)]
pub struct ImaAdpcmEncoder {
    index: i32,
}

#[inline]
fn decode_step(predictor: i32, index: i32, code: u8) -> (i32, i32) {
    let step = STEP_TABLE[index as usize];
    let mut diff = step >> 3;
    if code & 4 != 0 {
        diff += step;
    }
    if code & 2 != 0 {
        diff += step >> 1;
    }
    if code & 1 != 0 {
        diff += step >> 2;
    }
    let p = if code & 8 != 0 { predictor - diff } else { predictor + diff };
    let p = p.clamp(-32768, 32767);
    let i = (index + INDEX_TABLE[(code & 7) as usize]).clamp(0, 88);
    (p, i)
}

impl ImaAdpcmEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Encode one mono block. `samples.len()` is the samples-per-block value
    /// (for example 1529 or 1017); exactly `mono_block_align(len)` bytes are appended.
    pub fn encode_block(&mut self, samples: &[i16], out: &mut Vec<u8>) {
        assert!(samples.len() >= 2 && samples.len() % 2 == 1, "odd samples per block expected");
        let start = out.len();
        let first = samples[0] as i32;
        out.extend_from_slice(&(first as i16).to_le_bytes());
        out.push(self.index as u8);
        out.push(0);
        let mut predictor = first;
        let mut index = self.index;
        let mut pending: Option<u8> = None;
        for &s in &samples[1..] {
            let target = s as i32;
            // Greedy choice tested against all 16 codes keeps the error minimal
            // while matching the decoder bit for bit.
            let mut best = (i64::MAX, 0u8, predictor, index);
            for code in 0u8..16 {
                let (p, i) = decode_step(predictor, index, code);
                let err = (p - target) as i64;
                let cost = err * err;
                if cost < best.0 {
                    best = (cost, code, p, i);
                }
            }
            predictor = best.2;
            index = best.3;
            match pending.take() {
                None => pending = Some(best.1),
                Some(lo) => out.push(lo | (best.1 << 4)),
            }
        }
        debug_assert!(pending.is_none());
        self.index = index;
        debug_assert_eq!(out.len() - start, mono_block_align(samples.len()));
    }
}

/// Reference decoder used by tests and the preview path.
pub fn decode_mono_block(block: &[u8], samples_per_block: usize) -> Vec<i16> {
    let mut out = Vec::with_capacity(samples_per_block);
    let mut predictor = i16::from_le_bytes([block[0], block[1]]) as i32;
    let mut index = (block[2] as i32).clamp(0, 88);
    out.push(predictor as i16);
    for &byte in &block[4..] {
        for code in [byte & 0x0f, byte >> 4] {
            if out.len() == samples_per_block {
                break;
            }
            let (p, i) = decode_step(predictor, index, code);
            predictor = p;
            index = i;
            out.push(p as i16);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_sizes_match_classic_players() {
        assert_eq!(mono_block_align(1529), 768);
        assert_eq!(mono_block_align(1017), 512);
    }

    #[test]
    fn round_trip_sine_is_close() {
        let spb = 1017;
        let samples: Vec<i16> = (0..spb * 4)
            .map(|i| ((i as f64 * 0.05).sin() * 12000.0) as i16)
            .collect();
        let mut enc = ImaAdpcmEncoder::new();
        let mut decoded = Vec::new();
        for chunk in samples.chunks(spb) {
            let mut block = Vec::new();
            enc.encode_block(chunk, &mut block);
            assert_eq!(block.len(), 512);
            decoded.extend(decode_mono_block(&block, spb));
        }
        let err: f64 = samples
            .iter()
            .zip(decoded.iter())
            .skip(spb) // allow the step size to adapt during the first block
            .map(|(a, b)| ((*a as f64) - (*b as f64)).powi(2))
            .sum::<f64>()
            / (samples.len() - spb) as f64;
        assert!(err.sqrt() < 400.0, "rms error too high: {}", err.sqrt());
    }
}
