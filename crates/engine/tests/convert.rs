//! End-to-end tests: generate media with FFmpeg, run every available preset,
//! then verify each output by probing it and decoding it completely.
//!
//! Skipped (with a message) when no FFmpeg is installed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use pocketreel_engine::plan::{self, Settings};
use pocketreel_engine::presets::{self, Fit};
use pocketreel_engine::probe::{self, MediaInfo};
use pocketreel_engine::progress::{CancelToken, Progress};
use pocketreel_engine::queue::execute;
use pocketreel_engine::{preview, Tools};

fn tools() -> Option<&'static Tools> {
    static T: OnceLock<Option<Tools>> = OnceLock::new();
    T.get_or_init(|| Tools::locate(&[]).ok()).as_ref()
}

fn workdir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("pocketreel-it-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 6 s, 1280x720 @ 30000/1001, H.264 (or MPEG-4) + AAC stereo 48 kHz.
fn sample_video(t: &Tools, dir: &Path, name: &str, size: &str, audio: bool) -> PathBuf {
    let out = dir.join(name);
    if out.exists() {
        return out;
    }
    let vcodec = t.h264_encoder().unwrap_or("mpeg4");
    let mut c = Command::new(&t.ffmpeg);
    c.args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=size={size}:rate=30000/1001:duration=6"));
    if audio {
        c.args(["-f", "lavfi", "-i", "sine=frequency=330:sample_rate=48000:duration=6", "-ac", "2"]);
    }
    c.args(["-c:v", vcodec, "-pix_fmt", "yuv420p", "-b:v", "2M"]);
    if audio {
        c.args(["-c:a", "aac", "-b:a", "128k"]);
    }
    c.arg(&out);
    assert!(c.status().unwrap().success(), "failed to make sample");
    out
}

fn decode_ok(t: &Tools, file: &Path) -> (u64, f64) {
    // Count decoded video frames and audio seconds; any decode error fails.
    let v = Command::new(&t.ffmpeg)
        .args(["-v", "error", "-xerror", "-i"])
        .arg(file)
        .args(["-map", "0:v:0?", "-f", "framecrc", "-"])
        .output()
        .unwrap();
    assert!(v.status.success(), "video decode failed for {}: {}", file.display(), String::from_utf8_lossy(&v.stderr));
    let frames = String::from_utf8_lossy(&v.stdout).lines().filter(|l| !l.starts_with('#')).count() as u64;
    let a = Command::new(&t.ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-map", "0:a:0?", "-ac", "1", "-ar", "8000", "-f", "s16le", "-"])
        .output()
        .unwrap();
    assert!(a.status.success(), "audio decode failed for {}", file.display());
    (frames, a.stdout.len() as f64 / 2.0 / 8000.0)
}

fn convert(t: &Tools, input: &Path, settings: Settings, name: &str) -> (PathBuf, MediaInfo, plan::Plan) {
    let media = probe::probe(t, input).unwrap();
    let plan = plan::resolve(t, &settings, &media).unwrap();
    let out = workdir().join(format!("{name}.{}", plan.extension));
    let mut last: Option<Progress> = None;
    let bytes = execute(t, &plan, input, &out, true, &CancelToken::new(), &mut |p| last = Some(p)).unwrap();
    assert!(bytes > 0);
    assert_eq!(std::fs::metadata(&out).unwrap().len(), bytes);
    let last = last.expect("progress reported");
    assert_eq!(last.fraction, 1.0);
    (out, media, plan)
}

fn settings(preset: &str) -> Settings {
    Settings { preset: preset.into(), ..Default::default() }
}

macro_rules! need_tools {
    () => {
        match tools() {
            Some(t) => t,
            None => {
                eprintln!("FFmpeg not found; skipping");
                return;
            }
        }
    };
}

#[test]
fn classic_avi_every_size_and_rate() {
    let t = need_tools!();
    let dir = workdir();
    let input = sample_video(t, &dir, "src.mp4", "1280x720", true);
    for &(w, h) in presets::CLASSIC_SIZES.iter() {
        for fps in [22.0, 14.0] {
            let s = Settings { width: Some(w), height: Some(h), fps: Some(fps), ..settings("classic-avi") };
            let (out, _, plan) = convert(t, &input, s, &format!("classic-{w}x{h}-{fps}"));
            let m = probe::probe(t, &out).unwrap();
            let v = m.video.as_ref().unwrap();
            assert_eq!((v.codec.as_str(), v.width, v.height), ("mjpeg", w, h));
            let a = m.audio.as_ref().unwrap();
            assert_eq!((a.codec.as_str(), a.sample_rate, a.channels), ("adpcm_ima_wav", 22050, 1));
            let (frames, audio_secs) = decode_ok(t, &out);
            let expected = (6.0 * plan.video.as_ref().unwrap().fps).round() as i64;
            assert!((frames as i64 - expected).abs() <= 1, "{w}x{h}@{fps}: {frames} frames, expected {expected}");
            assert!((audio_secs - 6.0).abs() < 0.2, "audio {audio_secs}s");
            // Non-standard fields the players read.
            let data = std::fs::read(&out).unwrap();
            let rd = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
            let spb = if fps >= 18.0 { 1017 } else { 1529 };
            assert_eq!(rd(32 + 40), 22_579_200 / spb, "dwReserved[0]");
            assert_eq!(rd(32 + 52), 6, "dwReserved[3] = 00:00:06");
            assert_eq!(rd(32 + 16), frames as u32, "dwTotalFrames");
        }
    }
}

#[test]
fn classic_avi_letterbox_crop_trim_and_silent_source() {
    let t = need_tools!();
    let dir = workdir();
    let silent = sample_video(t, &dir, "silent.mp4", "640x480", false);
    let s = Settings { fit: Some(Fit::Letterbox), trim_start: Some(1.0), trim_end: Some(4.0), ..settings("classic-avi") };
    let (out, _, _) = convert(t, &silent, s, "classic-letterbox-trim");
    let (frames, audio) = decode_ok(t, &out);
    assert!((frames as i64 - 65).abs() <= 1, "3 s at 21.68 fps, got {frames}");
    assert!((audio - 3.0).abs() < 0.1, "silent track spans the clip");
    let s = Settings { fit: Some(Fit::Crop), ..settings("classic-avi") };
    let (out, _, _) = convert(t, &silent, s, "classic-crop");
    decode_ok(t, &out);
}

#[test]
fn every_ffmpeg_preset_produces_valid_files() {
    let t = need_tools!();
    let dir = workdir();
    let input = sample_video(t, &dir, "src.mp4", "1280x720", true);
    for p in presets::catalog_for(t) {
        if p.id == "classic-avi" {
            continue;
        }
        if !p.available {
            eprintln!("skip {} ({})", p.id, p.unavailable_reason.unwrap_or_default());
            continue;
        }
        let (out, _, plan) = convert(t, &input, settings(&p.id), &format!("preset-{}", p.id));
        let m = probe::probe(t, &out).unwrap();
        match &plan.video {
            Some(v) => {
                let got = m.video.as_ref().expect("video stream");
                assert_eq!((got.width, got.height), (v.width, v.height), "{}", p.id);
            }
            None => assert!(m.video.is_none(), "{} must be audio only", p.id),
        }
        let a = m.audio.as_ref().expect("audio stream");
        assert_eq!(a.sample_rate, plan.audio.as_ref().unwrap().sample_rate, "{}", p.id);
        let (frames, audio_secs) = decode_ok(t, &out);
        if plan.video.is_some() {
            assert!(frames > 100, "{}: {frames} frames", p.id);
        }
        assert!((audio_secs - 6.0).abs() < 0.3, "{}: audio {audio_secs}", p.id);
    }
}

#[test]
fn custom_size_fps_bitrate_and_mp3_extraction() {
    let t = need_tools!();
    if t.h264_encoder().is_none() {
        return;
    }
    let dir = workdir();
    let input = sample_video(t, &dir, "src.mp4", "1280x720", true);
    let s = Settings {
        width: Some(480),
        height: Some(480),
        fit: Some(Fit::Letterbox),
        fps: Some(15.0),
        video_bitrate_kbps: Some(600),
        audio_bitrate_kbps: Some(96),
        channels: Some(1),
        ..settings("mp4-720p")
    };
    let (out, _, _) = convert(t, &input, s, "custom-mp4");
    let m = probe::probe(t, &out).unwrap();
    let v = m.video.unwrap();
    assert_eq!((v.width, v.height), (480, 480));
    assert!((v.fps - 15.0).abs() < 0.01);
    assert_eq!(m.audio.unwrap().channels, 1);
    let kbps = m.bitrate.unwrap() as f64 / 1000.0;
    assert!(kbps < 900.0, "bitrate honoured, got {kbps}");

    let s = Settings { audio_bitrate_kbps: Some(320), ..settings("mp3") };
    let (out, _, _) = convert(t, &input, s, "extract");
    let m = probe::probe(t, &out).unwrap();
    assert_eq!(m.audio.as_ref().unwrap().codec, "mp3");
    assert!((m.duration - 6.0).abs() < 0.2);
}

#[test]
fn estimates_bracket_the_real_size() {
    let t = need_tools!();
    let dir = workdir();
    let input = sample_video(t, &dir, "src.mp4", "1280x720", true);
    for id in ["classic-avi", "wav", "mp3"] {
        let media = probe::probe(t, &input).unwrap();
        let plan = plan::resolve(t, &settings(id), &media).unwrap();
        let est = preview::estimate(t, &plan, &input).unwrap();
        let (out, _, _) = convert(t, &input, settings(id), &format!("estimate-{id}"));
        let real = std::fs::metadata(&out).unwrap().len();
        assert!(est.low <= real && real <= est.high, "{id}: real {real} not in {}..{}", est.low, est.high);
        let err = (est.bytes as f64 - real as f64).abs() / real as f64;
        assert!(err < 0.15, "{id}: estimate {} vs real {real}", est.bytes);
    }
}

#[test]
fn cancel_removes_partial_output() {
    let t = need_tools!();
    let dir = workdir();
    let input = sample_video(t, &dir, "src.mp4", "1280x720", true);
    for id in ["classic-avi", "mp3"] {
        let media = probe::probe(t, &input).unwrap();
        let plan = plan::resolve(t, &settings(id), &media).unwrap();
        let out = dir.join(format!("cancel-{id}.{}", plan.extension));
        let token = CancelToken::new();
        token.cancel();
        let r = execute(t, &plan, &input, &out, true, &token, &mut |_| {});
        assert!(matches!(r, Err(pocketreel_engine::Error::Canceled)), "{id}: {r:?}");
        assert!(!out.exists());
        assert!(!plan::partial_path(&out).exists());
    }
}

#[test]
fn previews_have_output_dimensions() {
    let t = need_tools!();
    let dir = workdir();
    let input = sample_video(t, &dir, "src.mp4", "1280x720", true);
    let media = probe::probe(t, &input).unwrap();
    let plan = plan::resolve(t, &settings("classic-avi"), &media).unwrap();
    let img = preview::output_preview(t, &media, &plan, 2.0).unwrap();
    assert_eq!((img.width, img.height), (160, 120));
    assert!(img.data_url.starts_with("data:image/jpeg;base64,/9j/"));
    let thumb = preview::thumbnail(t, &media, 1.0, 320, 180).unwrap();
    assert_eq!((thumb.width, thumb.height), (320, 180));
}
