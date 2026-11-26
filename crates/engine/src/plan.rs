//! Turning a preset + user settings + probed media into a concrete plan,
//! FFmpeg arguments, output names and size estimates.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::avi::{samples_per_block_for_fps, CLASSIC_SAMPLE_RATE};
use crate::error::{Error, Result};
use crate::presets::{self, AudioCodec, Fit, Preset, VideoCodec};
use crate::probe::MediaInfo;
use crate::tools::Tools;

/// What the user picked. Every field except `preset` is optional and falls back
/// to the preset default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub preset: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fit: Option<Fit>,
    /// Frames per second. `Some(0.0)` keeps the source rate.
    pub fps: Option<f64>,
    pub video_bitrate_kbps: Option<u32>,
    pub audio_bitrate_kbps: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub trim_start: Option<f64>,
    pub trim_end: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoPlan {
    pub codec: VideoCodec,
    pub encoder: String,
    /// Final frame size.
    pub width: u32,
    pub height: u32,
    /// Picture size inside the frame (differs from the frame for letterbox).
    pub inner_width: u32,
    pub inner_height: u32,
    pub fit: Fit,
    /// Output frame rate as a rational, `None` keeps the source rate.
    pub fps_rational: Option<(u32, u32)>,
    /// Frame rate the output will have (source rate when kept).
    pub fps: f64,
    pub bitrate_kbps: Option<u32>,
    pub samples_per_block: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPlan {
    pub codec: AudioCodec,
    pub encoder: String,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate: u32,
    pub channels: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub preset_id: String,
    pub preset_name: String,
    pub extension: String,
    pub classic: bool,
    pub start: f64,
    pub duration: f64,
    pub video: Option<VideoPlan>,
    pub audio: Option<AudioPlan>,
    pub source_has_audio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EstimateMethod {
    /// Size follows directly from the format (WAV).
    Exact,
    /// Derived from constant or target bitrates.
    Bitrate,
    /// Measured by encoding sample frames of this very file.
    Sampled,
    /// Typical compression ratio.
    Approximate,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    pub bytes: u64,
    pub low: u64,
    pub high: u64,
    pub method: EstimateMethod,
}

impl Estimate {
    pub fn range(bytes: f64, low: f64, high: f64, method: EstimateMethod) -> Self {
        Estimate { bytes: bytes.round() as u64, low: low.round() as u64, high: high.round() as u64, method }
    }
}

fn even(x: f64) -> u32 {
    (((x / 2.0).round() as u32) * 2).max(2)
}

/// Compute (frame w, frame h, picture w, picture h).
pub fn output_dims(fit: Fit, box_w: u32, box_h: u32, src_w: u32, src_h: u32, upscale: bool) -> (u32, u32, u32, u32) {
    let (bw, bh) = (box_w.max(2), box_h.max(2));
    let (sw, sh) = (src_w.max(1) as f64, src_h.max(1) as f64);
    match fit {
        Fit::Stretch | Fit::Crop => (bw, bh, bw, bh),
        Fit::Letterbox => {
            let s = (bw as f64 / sw).min(bh as f64 / sh);
            let iw = even(sw * s).min(bw);
            let ih = even(sh * s).min(bh);
            (bw, bh, iw, ih)
        }
        Fit::Contain => {
            let mut s = (bw as f64 / sw).min(bh as f64 / sh);
            if !upscale {
                s = s.min(1.0);
            }
            let w = even(sw * s);
            let h = even(sh * s);
            (w, h, w, h)
        }
    }
}

fn rational(fps: f64) -> (u32, u32) {
    let known = [(23.976, (24000, 1001)), (29.97, (30000, 1001)), (59.94, (60000, 1001))];
    for (v, r) in known {
        if (fps - v).abs() < 0.005 {
            return r;
        }
    }
    let den = 1000u32;
    ((fps * den as f64).round() as u32, den)
}

fn video_encoder(codec: VideoCodec, tools: &Tools) -> Result<String> {
    let name = match codec {
        VideoCodec::ClassicMjpeg => return Ok("pocketreel-mjpeg".into()),
        VideoCodec::Amv => "amv",
        VideoCodec::Xvid => "mpeg4",
        VideoCodec::Vp9 => "libvpx-vp9",
        VideoCodec::H264 => tools.h264_encoder().ok_or_else(|| Error::EncoderMissing("H.264".into()))?,
    };
    if !tools.has_encoder(name) {
        return Err(Error::EncoderMissing(name.into()));
    }
    Ok(name.to_string())
}

fn audio_encoder(codec: AudioCodec, tools: &Tools) -> Result<String> {
    let name = match codec {
        AudioCodec::ClassicAdpcm => return Ok("pocketreel-ima-adpcm".into()),
        AudioCodec::AmvAdpcm => "adpcm_ima_amv",
        AudioCodec::Aac => "aac",
        AudioCodec::Mp3 => "libmp3lame",
        AudioCodec::Opus => "libopus",
        AudioCodec::Pcm => "pcm_s16le",
        AudioCodec::Flac => "flac",
    };
    if !tools.has_encoder(name) {
        return Err(Error::EncoderMissing(name.into()));
    }
    Ok(name.to_string())
}

/// Resolve settings against a preset and a probed file.
pub fn resolve(tools: &Tools, settings: &Settings, media: &MediaInfo) -> Result<Plan> {
    let preset: Preset = presets::find(&settings.preset).ok_or_else(|| Error::UnknownPreset(settings.preset.clone()))?;

    let total = media.duration.max(0.0);
    let start = settings.trim_start.unwrap_or(0.0).clamp(0.0, total);
    let end = settings.trim_end.filter(|e| *e > 0.0).unwrap_or(total).min(total);
    if total > 0.0 && end - start < 0.1 {
        return Err(Error::Invalid("The trimmed part is too short. Choose an end time after the start time.".into()));
    }
    let duration = if total > 0.0 { end - start } else { 0.0 };

    let video = match &preset.video {
        None => None,
        Some(vp) => {
            let src = media.video.as_ref().ok_or(Error::MissingStream("video"))?;
            let fit = settings.fit.unwrap_or(vp.fit);
            if !vp.fits.contains(&fit) {
                return Err(Error::Invalid("That framing option is not available for this preset.".into()));
            }
            let bw = settings.width.unwrap_or(vp.width);
            let bh = settings.height.unwrap_or(vp.height);
            if !(16..=7680).contains(&bw) || !(16..=4320).contains(&bh) {
                return Err(Error::Invalid("Width and height must be between 16 and 7680 × 4320 pixels.".into()));
            }
            if !vp.custom_size && !vp.sizes.iter().any(|s| s.width == bw && s.height == bh) {
                return Err(Error::Invalid(format!("{bw} × {bh} is not supported by this format.")));
            }
            let (bw, bh) = (bw + bw % 2, bh + bh % 2);
            let (w, h, iw, ih) = output_dims(fit, bw, bh, src.display_width, src.display_height, vp.upscale);

            let requested = settings.fps.or(vp.fps);
            let (fps_rational, fps, spb) = match vp.codec {
                VideoCodec::ClassicMjpeg => {
                    let spb = samples_per_block_for_fps(requested.unwrap_or(22.0));
                    (Some((CLASSIC_SAMPLE_RATE, spb)), CLASSIC_SAMPLE_RATE as f64 / spb as f64, Some(spb))
                }
                VideoCodec::Amv => {
                    let f = requested.unwrap_or(25.0);
                    if !vp.fps_options.iter().any(|o| (o.value - f).abs() < 0.01) {
                        return Err(Error::Invalid("AMV supports 10, 15, 25 or 30 fps.".into()));
                    }
                    (Some((f as u32, 1)), f, None)
                }
                _ => match requested.filter(|f| *f > 0.0) {
                    Some(f) => {
                        if !(1.0..=120.0).contains(&f) {
                            return Err(Error::Invalid("Frame rate must be between 1 and 120 fps.".into()));
                        }
                        let r = rational(f);
                        (Some(r), r.0 as f64 / r.1 as f64, None)
                    }
                    None => (None, if src.fps > 0.0 { src.fps } else { 30.0 }, None),
                },
            };
            let bitrate_kbps = match vp.bitrate_kbps {
                None => None,
                Some(def) => {
                    let b = settings.video_bitrate_kbps.unwrap_or(def);
                    if b < vp.bitrate_min || b > vp.bitrate_max {
                        return Err(Error::Invalid(format!(
                            "Video bitrate must be between {} and {} kbps.",
                            vp.bitrate_min, vp.bitrate_max
                        )));
                    }
                    Some(b)
                }
            };
            Some(VideoPlan {
                codec: vp.codec,
                encoder: video_encoder(vp.codec, tools)?,
                width: w,
                height: h,
                inner_width: iw,
                inner_height: ih,
                fit,
                fps_rational,
                fps,
                bitrate_kbps,
                samples_per_block: spb,
            })
        }
    };

    let audio = match &preset.audio {
        None => None,
        Some(ap) => {
            if preset.video.is_none() && media.audio.is_none() {
                return Err(Error::MissingStream("audio"));
            }
            let src_ch = media.audio.as_ref().map(|a| a.channels).unwrap_or(2);
            let (sample_rate, channels, bitrate_kbps) = if ap.editable {
                let sr = settings.sample_rate.unwrap_or(ap.sample_rate);
                if !ap.sample_rates.contains(&sr) {
                    return Err(Error::Invalid(format!("{sr} Hz is not available for this format.")));
                }
                let ch = settings.channels.unwrap_or(ap.channels.min(src_ch.max(1)));
                if !(1..=2).contains(&ch) {
                    return Err(Error::Invalid("Choose mono or stereo.".into()));
                }
                let br = match ap.bitrate_kbps {
                    None => None,
                    Some(def) => {
                        let b = settings.audio_bitrate_kbps.unwrap_or(def);
                        if !(32..=512).contains(&b) {
                            return Err(Error::Invalid("Audio bitrate must be between 32 and 512 kbps.".into()));
                        }
                        Some(b)
                    }
                };
                (sr, ch, br)
            } else {
                (ap.sample_rate, ap.channels, ap.bitrate_kbps)
            };
            Some(AudioPlan { codec: ap.codec, encoder: audio_encoder(ap.codec, tools)?, bitrate_kbps, sample_rate, channels })
        }
    };

    if let Some(v) = &video {
        if v.codec == VideoCodec::Amv && !tools.has_muxer("amv") {
            return Err(Error::EncoderMissing("AMV muxer".into()));
        }
    }

    Ok(Plan {
        preset_id: preset.id.clone(),
        preset_name: preset.name.clone(),
        extension: preset.extension.clone(),
        classic: matches!(video.as_ref().map(|v| v.codec), Some(VideoCodec::ClassicMjpeg)),
        start,
        duration,
        video,
        audio,
        source_has_audio: media.audio.is_some(),
    })
}

/// FFmpeg video filter chain producing exactly the planned frames.
pub fn video_filter(v: &VideoPlan) -> String {
    let flags = if matches!(v.codec, VideoCodec::ClassicMjpeg | VideoCodec::Amv) { "bicubic" } else { "lanczos" };
    let mut f = Vec::new();
    if let Some((n, d)) = v.fps_rational {
        f.push(format!("fps={n}/{d}"));
    }
    match v.fit {
        Fit::Stretch | Fit::Contain => f.push(format!("scale={}:{}:flags={flags}", v.width, v.height)),
        Fit::Crop => {
            f.push(format!(
                "scale={}:{}:force_original_aspect_ratio=increase:flags={flags}",
                v.width, v.height
            ));
            f.push(format!("crop={}:{}", v.width, v.height));
        }
        Fit::Letterbox => {
            f.push(format!("scale={}:{}:flags={flags}", v.inner_width, v.inner_height));
            f.push(format!("pad={}:{}:(ow-iw)/2:(oh-ih)/2:color=black", v.width, v.height));
        }
    }
    f.push("setsar=1".into());
    f.join(",")
}

fn muxer(plan: &Plan) -> &'static str {
    match plan.extension.as_str() {
        "mp4" => "mp4",
        "m4a" => "ipod",
        "webm" => "webm",
        "amv" => "amv",
        "avi" => "avi",
        "mp3" => "mp3",
        "flac" => "flac",
        "wav" => "wav",
        _ => "matroska",
    }
}

fn s<T: ToString>(v: T) -> OsString {
    OsString::from(v.to_string())
}

/// Arguments for a direct FFmpeg conversion (every preset except the classic AVI,
/// which is encoded in-process).
pub fn ffmpeg_args(plan: &Plan, input: &Path, output: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-progress", "pipe:1", "-nostats"]
        .iter()
        .map(OsString::from)
        .collect();
    if plan.start > 0.0 {
        a.extend([s("-ss"), s(format!("{:.3}", plan.start))]);
    }
    a.extend([s("-i"), input.as_os_str().to_owned()]);
    let amv = plan.video.as_ref().map(|v| v.codec == VideoCodec::Amv).unwrap_or(false);
    let synth_audio = amv && !plan.source_has_audio;
    if synth_audio {
        a.extend([s("-f"), s("lavfi"), s("-i"), s("anullsrc=r=22050:cl=mono")]);
    }
    if plan.duration > 0.0 {
        a.extend([s("-t"), s(format!("{:.3}", plan.duration))]);
    }

    match &plan.video {
        Some(v) => {
            a.extend([s("-map"), s("0:v:0"), s("-vf"), s(video_filter(v))]);
            match v.codec {
                VideoCodec::H264 => {
                    a.extend([s("-c:v"), s(&v.encoder), s("-pix_fmt"), s("yuv420p")]);
                    if v.encoder == "libx264" {
                        a.extend([s("-preset"), s("medium"), s("-profile:v"), s("high")]);
                    }
                }
                VideoCodec::Xvid => a.extend([
                    s("-c:v"), s("mpeg4"), s("-vtag"), s("XVID"), s("-pix_fmt"), s("yuv420p"),
                    s("-mbd"), s("rd"), s("-flags"), s("+mv4+aic"), s("-trellis"), s("2"),
                    s("-cmp"), s("2"), s("-subcmp"), s("2"), s("-g"), s("300"), s("-bf"), s("2"),
                ]),
                VideoCodec::Vp9 => a.extend([
                    s("-c:v"), s("libvpx-vp9"), s("-pix_fmt"), s("yuv420p"), s("-deadline"), s("good"),
                    s("-cpu-used"), s("4"), s("-row-mt"), s("1"),
                ]),
                VideoCodec::Amv => a.extend([s("-c:v"), s("amv"), s("-pix_fmt"), s("yuvj420p"), s("-strict"), s("-2")]),
                VideoCodec::ClassicMjpeg => {}
            }
            if let Some(k) = v.bitrate_kbps {
                a.extend([
                    s("-b:v"),
                    s(format!("{k}k")),
                    s("-maxrate"),
                    s(format!("{}k", k * 3 / 2)),
                    s("-bufsize"),
                    s(format!("{}k", k * 2)),
                ]);
            }
        }
        None => a.push(s("-vn")),
    }

    match &plan.audio {
        Some(au) if plan.source_has_audio || synth_audio => {
            a.extend([s("-map"), s(if synth_audio { "1:a:0" } else { "0:a:0" })]);
            a.extend([s("-c:a"), s(&au.encoder), s("-ar"), s(au.sample_rate), s("-ac"), s(au.channels)]);
            if let Some(k) = au.bitrate_kbps {
                a.extend([s("-b:a"), s(format!("{k}k"))]);
            }
            if au.codec == AudioCodec::AmvAdpcm {
                let fps = plan.video.as_ref().map(|v| v.fps).unwrap_or(25.0);
                a.extend([s("-block_size"), s((au.sample_rate as f64 / fps).round() as u32)]);
            }
            if au.codec == AudioCodec::Flac {
                a.extend([s("-compression_level"), s("8")]);
            }
        }
        _ => a.push(s("-an")),
    }

    if synth_audio {
        a.push(s("-shortest"));
    }
    a.extend([s("-map_metadata"), s("0"), s("-map_chapters"), s("-1")]);
    match plan.extension.as_str() {
        "mp4" | "m4a" => a.extend([s("-movflags"), s("+faststart")]),
        "mp3" => a.extend([s("-id3v2_version"), s("3"), s("-write_xing"), s("1")]),
        _ => {}
    }
    a.extend([s("-f"), s(muxer(plan)), output.as_os_str().to_owned()]);
    a
}

/// Estimate for every preset that does not need sampling.
pub fn estimate_by_formula(plan: &Plan) -> Option<Estimate> {
    let d = plan.duration;
    let audio_bytes = |au: &AudioPlan| -> Option<f64> {
        match au.codec {
            AudioCodec::Pcm => Some(d * au.sample_rate as f64 * au.channels as f64 * 2.0),
            AudioCodec::Flac => None,
            _ => au.bitrate_kbps.map(|k| k as f64 * 1000.0 / 8.0 * d),
        }
    };
    match (&plan.video, &plan.audio) {
        (None, Some(au)) => match au.codec {
            AudioCodec::Pcm => {
                // 44-byte header plus a small metadata chunk (title, encoder).
                let data = audio_bytes(au)? + 44.0;
                // The container's length can run a codec frame (up to 2048 samples)
                // past the audio itself.
                let frame = 2048.0 * au.channels as f64 * 2.0;
                Some(Estimate::range(data + 64.0, data - frame, data + 4096.0, EstimateMethod::Exact))
            }
            AudioCodec::Flac => {
                let wav = d * au.sample_rate as f64 * au.channels as f64 * 2.0;
                Some(Estimate::range(wav * 0.55, wav * 0.35, wav * 0.75, EstimateMethod::Approximate))
            }
            AudioCodec::Mp3 => {
                // Frames at constant bitrate plus the ID3v2 tag and the Xing info frame.
                let b = audio_bytes(au)? + 1_600.0;
                Some(Estimate::range(b, b * 0.98, b * 1.02 + 8_192.0, EstimateMethod::Bitrate))
            }
            _ => {
                let b = audio_bytes(au)? * 1.01 + 2_000.0;
                Some(Estimate::range(b, b * 0.9, b * 1.05, EstimateMethod::Bitrate))
            }
        },
        (Some(v), au) => {
            let vk = v.bitrate_kbps? as f64;
            let video = vk * 1000.0 / 8.0 * d;
            let audio = if plan.source_has_audio { au.as_ref().and_then(audio_bytes).unwrap_or(0.0) } else { 0.0 };
            let overhead = if plan.extension == "avi" { 1.015 } else { 1.006 };
            let b = (video + audio) * overhead + 4_096.0;
            // Rate control hits the target closely on busy footage but undershoots on static scenes.
            Some(Estimate::range(b, video * 0.55 + audio, b * 1.06, EstimateMethod::Bitrate))
        }
        (None, None) => None,
    }
}

/// Choose an output path that does not overwrite anything unless allowed.
pub fn output_path(input: &Path, out_dir: Option<&Path>, extension: &str, overwrite: bool) -> PathBuf {
    let dir = out_dir
        .map(Path::to_path_buf)
        .or_else(|| input.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "video".into());
    let mut candidate = dir.join(format!("{stem}.{extension}"));
    if candidate == input {
        candidate = dir.join(format!("{stem} (converted).{extension}"));
    }
    if overwrite && candidate != input {
        return candidate;
    }
    let mut n = 2;
    while candidate.exists() || candidate == input {
        candidate = dir.join(format!("{stem} ({n}).{extension}"));
        n += 1;
    }
    candidate
}

/// Hidden temporary sibling used while a conversion runs. It keeps the real
/// extension because Android only allows media file types in Movies/ and Music/.
pub fn partial_path(output: &Path) -> PathBuf {
    let stem = output.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    match output.extension() {
        Some(ext) => output.with_file_name(format!(".{stem}.partial.{}", ext.to_string_lossy())),
        None => output.with_file_name(format!(".{stem}.partial")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dims_for_each_fit() {
        assert_eq!(output_dims(Fit::Stretch, 160, 120, 1920, 1080, true), (160, 120, 160, 120));
        assert_eq!(output_dims(Fit::Letterbox, 160, 120, 1920, 1080, true), (160, 120, 160, 90));
        assert_eq!(output_dims(Fit::Crop, 160, 120, 1920, 1080, true), (160, 120, 160, 120));
        assert_eq!(output_dims(Fit::Contain, 1280, 720, 1920, 1080, false), (1280, 720, 1280, 720));
        assert_eq!(output_dims(Fit::Contain, 1280, 720, 640, 360, false), (640, 360, 640, 360));
        assert_eq!(output_dims(Fit::Contain, 1280, 720, 1080, 1920, false), (406, 720, 406, 720));
    }

    #[test]
    fn rational_rates() {
        assert_eq!(rational(29.97), (30000, 1001));
        assert_eq!(rational(25.0), (25000, 1000));
    }

    #[test]
    fn output_names_never_clobber_input() {
        let p = output_path(Path::new("/tmp/a/movie.avi"), None, "avi", true);
        assert_eq!(p, PathBuf::from("/tmp/a/movie (converted).avi"));
        let p = output_path(Path::new("/nonexistent/x/movie.mp4"), None, "avi", false);
        assert_eq!(p, PathBuf::from("/nonexistent/x/movie.avi"));
        assert_eq!(partial_path(Path::new("/x/movie.avi")), PathBuf::from("/x/.movie.partial.avi"));
    }
}
