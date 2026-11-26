//! Byte-for-byte fidelity with the original converter.
//!
//! The expected hashes were computed from the header templates inside the
//! original `AVI_EncDLL.dll`, patched field by field exactly as its
//! `AviEncoderInit` / `AviEncoderClose` routines do (see docs/FINDINGS.md).
//! Only the hashes are stored here; no bytes of the original are.

use pocketreel_engine::avi::{build_header, pack_hhmmss, ClassicParams};
use pocketreel_engine::jpeg::JpegEncoder;
use sha2::{Digest, Sha256};

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn jpeg_frame_header_matches_original() {
    let h = JpegEncoder::new(160, 120, 75).header().to_vec();
    assert_eq!(hex(&h), "a048d7bfad872a42f6896b6fa02af048923e53635e7c05b79097aef5f1ea3365");
    let h = JpegEncoder::new(208, 176, 75).header().to_vec();
    assert_eq!(hex(&h), "455ef0b871aeb2aa0c23b99b1e326a0e11cccb9cd2116cce9e82d9e96f8f3fea");
}

#[test]
fn avi_header_matches_original() {
    let p = ClassicParams::new(160, 120, 1017);
    let h = build_header(&p, 100, 123_456, 3200, pack_hhmmss(65));
    assert_eq!(hex(&h), "2b15b005382a46ce0f7ba67c6d245f6b7ef1899f851b262937b70f77c048b4af");
    let p = ClassicParams::new(320, 240, 1529);
    let h = build_header(&p, 7, 9999, 224, 0x01_0203);
    assert_eq!(hex(&h), "4ffaeab07d468acc44dd292919656d280cf20423fec68d600e944b5439098e62");
}
