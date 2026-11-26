//! Thumbnails, output previews and size estimates.

use std::path::Path;

use base64::Engine as _;
use serde::Serialize;

use crate::classic::{self, grab_rgb, still_filter, CLASSIC_QUALITY};
use crate::error::{Error, Result};
use crate::jpeg::JpegEncoder;
use crate::plan::{self, output_dims, Estimate, EstimateMethod, Plan};
use crate::presets::{AudioCodec, Fit, VideoCodec};
use crate::probe::MediaInfo;
use crate::tools::Tools;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `data:image/jpeg;base64,...`
    pub data_url: String,
}

fn jpeg_data_url(rgb: &[u8], w: u32, h: u32, quality: u8) -> String {
    let mut enc = JpegEncoder::new(w, h, quality);
    let mut out = Vec::new();
    enc.encode(rgb, &mut out);
    format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out))
}

/// A thumbnail of the source that fits inside `max_w` x `max_h`.
pub fn thumbnail(tools: &Tools, media: &MediaInfo, at: f64, max_w: u32, max_h: u32) -> Result<Image> {
    let v = media.video.as_ref().ok_or(Error::MissingStream("video"))?;
    let (w, h, _, _) = output_dims(Fit::Contain, max_w, max_h, v.display_width, v.display_height, false);
    let vf = format!("scale={w}:{h}:flags=bicubic,setsar=1");
    let rgb = grab_rgb(tools, Path::new(&media.path), at, &vf, w, h)?;
    Ok(Image { width: w, height: h, data_url: jpeg_data_url(&rgb, w, h, 86) })
}

/// One frame exactly as the output will frame it. For the classic AVI preset this
/// is the real output frame, compressed with the same encoder and tables.
pub fn output_preview(tools: &Tools, media: &MediaInfo, plan: &Plan, at: f64) -> Result<Image> {
    let v = plan.video.as_ref().ok_or(Error::MissingStream("video"))?;
    let rgb = grab_rgb(tools, Path::new(&media.path), at, &still_filter(v), v.width, v.height)?;
    let quality = if v.codec == VideoCodec::ClassicMjpeg { CLASSIC_QUALITY } else { 92 };
    Ok(Image { width: v.width, height: v.height, data_url: jpeg_data_url(&rgb, v.width, v.height, quality) })
}

/// Encode short samples with FFmpeg and extrapolate (AMV, FLAC).
fn sampled_ffmpeg(tools: &Tools, plan: &Plan, input: &Path) -> Result<Estimate> {
    let seg = plan.duration.min(2.0);
    let n = if plan.duration > 8.0 { 3 } else { 1 };
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n_dir = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("pocketreel-est-{}-{n_dir}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let mut rates = Vec::new();
    for i in 0..n {
        let mut p = plan.clone();
        p.start = plan.start + (plan.duration - seg) * (i as f64 + 0.5) / n as f64;
        p.duration = seg;
        let out = dir.join(format!("s{i}.{}", plan.extension));
        let mut args = plan::ffmpeg_args(&p, input, &out);
        // Drop "-progress pipe:1" for a quiet run.
        if let Some(pos) = args.iter().position(|a| a == "-progress") {
            args.drain(pos..pos + 2);
        }
        let ok = tools.ffmpeg_cmd().args(&args).output().map(|o| o.status.success()).unwrap_or(false);
        if ok {
            if let Ok(m) = std::fs::metadata(&out) {
                rates.push(m.len() as f64 / seg);
            }
        }
        let _ = std::fs::remove_file(&out);
    }
    let _ = std::fs::remove_dir(&dir);
    if rates.is_empty() {
        return Err(Error::Ffmpeg("Could not encode a sample to estimate the size.".into()));
    }
    let avg = rates.iter().sum::<f64>() / rates.len() as f64;
    let min = rates.iter().cloned().fold(f64::MAX, f64::min);
    let max = rates.iter().cloned().fold(0.0, f64::max);
    let d = plan.duration;
    Ok(Estimate::range(avg * d, min * d * 0.9, max * d * 1.1, EstimateMethod::Sampled))
}

/// Size estimate for any plan.
pub fn estimate(tools: &Tools, plan: &Plan, input: &Path) -> Result<Estimate> {
    if plan.classic {
        return classic::estimate(tools, plan, input);
    }
    let needs_sampling = plan.video.as_ref().map(|v| v.codec == VideoCodec::Amv).unwrap_or(false)
        || (plan.video.is_none() && plan.audio.as_ref().map(|a| a.codec == AudioCodec::Flac).unwrap_or(false));
    if needs_sampling {
        if let Ok(e) = sampled_ffmpeg(tools, plan, input) {
            return Ok(e);
        }
    }
    plan::estimate_by_formula(plan).ok_or_else(|| Error::Invalid("No estimate available.".into()))
}
