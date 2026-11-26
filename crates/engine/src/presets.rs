//! The preset catalog.

use serde::{Deserialize, Serialize};

use crate::tools::Tools;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    /// Scale to exactly the target size, ignoring aspect ratio (the original tool).
    Stretch,
    /// Keep aspect ratio and add black bars to reach the exact target size.
    Letterbox,
    /// Keep aspect ratio and crop the edges to fill the exact target size.
    Crop,
    /// Keep aspect ratio inside the target box; never upscale; no bars.
    Contain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VideoCodec {
    ClassicMjpeg,
    Amv,
    H264,
    Xvid,
    Vp9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioCodec {
    ClassicAdpcm,
    AmvAdpcm,
    Aac,
    Mp3,
    Opus,
    Pcm,
    Flac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Classic,
    Modern,
    Audio,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeOption {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsOption {
    /// Value sent back in settings.
    pub value: f64,
    /// Frame rate actually produced.
    pub actual: f64,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoPreset {
    pub codec: VideoCodec,
    pub width: u32,
    pub height: u32,
    pub sizes: Vec<SizeOption>,
    pub custom_size: bool,
    pub fit: Fit,
    pub fits: Vec<Fit>,
    /// `None` keeps the source frame rate.
    pub fps: Option<f64>,
    pub fps_options: Vec<FpsOption>,
    pub custom_fps: bool,
    pub bitrate_kbps: Option<u32>,
    pub bitrate_min: u32,
    pub bitrate_max: u32,
    /// Allow making the picture larger than the source.
    pub upscale: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPreset {
    pub codec: AudioCodec,
    pub bitrate_kbps: Option<u32>,
    pub bitrate_options: Vec<u32>,
    pub sample_rate: u32,
    pub sample_rates: Vec<u32>,
    pub channels: u32,
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub description: String,
    pub format_label: String,
    pub extension: String,
    pub video: Option<VideoPreset>,
    pub audio: Option<AudioPreset>,
    /// Encoders this preset needs.
    pub requires: Vec<String>,
    pub available: bool,
    pub unavailable_reason: Option<String>,
    /// True for the preset that reproduces the original tool exactly.
    pub original: bool,
}

/// Resolutions offered by the original tool (`AVISIZE` / `VIDEOSIZE`).
pub const CLASSIC_SIZES: [(u32, u32); 12] = [
    (96, 64),
    (128, 96),
    (128, 128),
    (160, 120),
    (160, 128),
    (176, 128),
    (176, 144),
    (192, 160),
    (208, 120),
    (208, 160),
    (208, 176),
    (320, 240),
];

fn sizes(list: &[(u32, u32)]) -> Vec<SizeOption> {
    list.iter().map(|&(width, height)| SizeOption { width, height }).collect()
}

fn keep_or(list: &[f64]) -> Vec<FpsOption> {
    list.iter()
        .map(|&f| FpsOption { value: f, actual: f, label: format!("{} fps", trim_float(f)) })
        .collect()
}

pub fn trim_float(f: f64) -> String {
    let s = format!("{:.3}", f);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn modern_video(codec: VideoCodec, w: u32, h: u32, kbps: u32) -> VideoPreset {
    VideoPreset {
        codec,
        width: w,
        height: h,
        sizes: sizes(&[(3840, 2160), (2560, 1440), (1920, 1080), (1280, 720), (854, 480), (640, 360), (426, 240)]),
        custom_size: true,
        fit: Fit::Contain,
        fits: vec![Fit::Contain, Fit::Letterbox, Fit::Crop, Fit::Stretch],
        fps: None,
        fps_options: keep_or(&[24.0, 25.0, 30.0, 50.0, 60.0]),
        custom_fps: true,
        bitrate_kbps: Some(kbps),
        bitrate_min: 100,
        bitrate_max: 50_000,
        upscale: false,
    }
}

fn audio(codec: AudioCodec, kbps: Option<u32>, options: &[u32], rate: u32, ch: u32) -> AudioPreset {
    AudioPreset {
        codec,
        bitrate_kbps: kbps,
        bitrate_options: options.to_vec(),
        sample_rate: rate,
        sample_rates: vec![22050, 32000, 44100, 48000],
        channels: ch,
        editable: true,
    }
}

/// The full catalog, without availability information.
pub fn catalog() -> Vec<Preset> {
    let classic_fps = vec![
        FpsOption { value: 22.0, actual: 22050.0 / 1017.0, label: "21.68 fps (High)".into() },
        FpsOption { value: 14.0, actual: 22050.0 / 1529.0, label: "14.42 fps (Low)".into() },
    ];
    let mut list = vec![
        Preset {
            id: "classic-avi".into(),
            name: "Classic player AVI".into(),
            category: Category::Classic,
            description: "The exact format of the original Actions converter: MJPEG video with IMA ADPCM audio, \
                          for small MP3/MP4 players."
                .into(),
            format_label: "AVI · MJPEG + IMA ADPCM, 22 kHz mono".into(),
            extension: "avi".into(),
            video: Some(VideoPreset {
                codec: VideoCodec::ClassicMjpeg,
                width: 160,
                height: 120,
                sizes: sizes(&CLASSIC_SIZES),
                custom_size: true,
                fit: Fit::Stretch,
                fits: vec![Fit::Stretch, Fit::Letterbox, Fit::Crop],
                fps: Some(22.0),
                fps_options: classic_fps,
                custom_fps: false,
                bitrate_kbps: None,
                bitrate_min: 0,
                bitrate_max: 0,
                upscale: true,
            }),
            audio: Some(AudioPreset {
                codec: AudioCodec::ClassicAdpcm,
                bitrate_kbps: None,
                bitrate_options: vec![],
                sample_rate: 22050,
                sample_rates: vec![22050],
                channels: 1,
                editable: false,
            }),
            requires: vec![],
            available: true,
            unavailable_reason: None,
            original: true,
        },
        Preset {
            id: "classic-amv".into(),
            name: "Classic player AMV".into(),
            category: Category::Classic,
            description: "AMV for players that only accept .amv files. Encoded with FFmpeg's AMV encoder.".into(),
            format_label: "AMV · AMV video + IMA ADPCM, 22 kHz mono".into(),
            extension: "amv".into(),
            video: Some(VideoPreset {
                codec: VideoCodec::Amv,
                width: 160,
                height: 120,
                sizes: sizes(&CLASSIC_SIZES),
                custom_size: false,
                fit: Fit::Letterbox,
                fits: vec![Fit::Letterbox, Fit::Crop, Fit::Stretch],
                fps: Some(25.0),
                fps_options: keep_or(&[30.0, 25.0, 15.0, 10.0]),
                custom_fps: false,
                bitrate_kbps: None,
                bitrate_min: 0,
                bitrate_max: 0,
                upscale: true,
            }),
            audio: Some(AudioPreset {
                codec: AudioCodec::AmvAdpcm,
                bitrate_kbps: None,
                bitrate_options: vec![],
                sample_rate: 22050,
                sample_rates: vec![22050],
                channels: 1,
                editable: false,
            }),
            requires: vec!["amv".into(), "adpcm_ima_amv".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "dvd-xvid".into(),
            name: "DVD & DivX player".into(),
            category: Category::Classic,
            description: "Xvid-compatible MPEG-4 AVI with MP3 audio for DVD players, car screens and older TVs.".into(),
            format_label: "AVI · MPEG-4 ASP (XVID) + MP3".into(),
            extension: "avi".into(),
            video: Some(VideoPreset {
                sizes: sizes(&[(720, 576), (720, 480), (640, 480), (640, 360), (480, 272), (320, 240)]),
                fps_options: keep_or(&[23.976, 25.0, 29.97]),
                bitrate_max: 8_000,
                ..modern_video(VideoCodec::Xvid, 720, 576, 1_800)
            }),
            audio: Some(audio(AudioCodec::Mp3, Some(160), &[96, 128, 160, 192, 256, 320], 44100, 2)),
            requires: vec!["mpeg4".into(), "libmp3lame".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "mp4-720p".into(),
            name: "Phone & tablet".into(),
            category: Category::Modern,
            description: "MP4 with H.264 and AAC. Plays on every phone, tablet, TV and browser.".into(),
            format_label: "MP4 · H.264 + AAC, up to 720p".into(),
            extension: "mp4".into(),
            video: Some(modern_video(VideoCodec::H264, 1280, 720, 2_500)),
            audio: Some(audio(AudioCodec::Aac, Some(128), &[64, 96, 128, 160, 192, 256], 48000, 2)),
            requires: vec!["h264".into(), "aac".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "mp4-1080p".into(),
            name: "Full HD".into(),
            category: Category::Modern,
            description: "High-quality MP4 for large screens and archiving.".into(),
            format_label: "MP4 · H.264 + AAC, up to 1080p".into(),
            extension: "mp4".into(),
            video: Some(modern_video(VideoCodec::H264, 1920, 1080, 6_000)),
            audio: Some(audio(AudioCodec::Aac, Some(192), &[96, 128, 160, 192, 256, 320], 48000, 2)),
            requires: vec!["h264".into(), "aac".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "mp4-small".into(),
            name: "Small file".into(),
            category: Category::Modern,
            description: "Compact 480p MP4 for messaging apps and email.".into(),
            format_label: "MP4 · H.264 + AAC, up to 480p".into(),
            extension: "mp4".into(),
            video: Some(modern_video(VideoCodec::H264, 854, 480, 900)),
            audio: Some(audio(AudioCodec::Aac, Some(96), &[64, 96, 128], 44100, 2)),
            requires: vec!["h264".into(), "aac".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "webm".into(),
            name: "Web video".into(),
            category: Category::Modern,
            description: "Open WebM format with VP9 and Opus, ideal for websites.".into(),
            format_label: "WebM · VP9 + Opus, up to 720p".into(),
            extension: "webm".into(),
            video: Some(modern_video(VideoCodec::Vp9, 1280, 720, 1_800)),
            audio: Some(AudioPreset {
                sample_rates: vec![48000],
                ..audio(AudioCodec::Opus, Some(128), &[64, 96, 128, 160, 192], 48000, 2)
            }),
            requires: vec!["libvpx-vp9".into(), "libopus".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "mp3".into(),
            name: "MP3 audio".into(),
            category: Category::Audio,
            description: "Extract the soundtrack as an MP3 for any music player.".into(),
            format_label: "MP3 · LAME, constant bitrate".into(),
            extension: "mp3".into(),
            video: None,
            audio: Some(audio(AudioCodec::Mp3, Some(192), &[96, 128, 160, 192, 256, 320], 44100, 2)),
            requires: vec!["libmp3lame".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "m4a".into(),
            name: "AAC audio".into(),
            category: Category::Audio,
            description: "Efficient AAC audio in an M4A file, great for phones.".into(),
            format_label: "M4A · AAC".into(),
            extension: "m4a".into(),
            video: None,
            audio: Some(audio(AudioCodec::Aac, Some(160), &[96, 128, 160, 192, 256], 44100, 2)),
            requires: vec!["aac".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "flac".into(),
            name: "FLAC audio".into(),
            category: Category::Audio,
            description: "Lossless compressed audio. Bit-perfect, about half the size of WAV.".into(),
            format_label: "FLAC · lossless".into(),
            extension: "flac".into(),
            video: None,
            audio: Some(audio(AudioCodec::Flac, None, &[], 44100, 2)),
            requires: vec!["flac".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
        Preset {
            id: "wav".into(),
            name: "WAV audio".into(),
            category: Category::Audio,
            description: "Uncompressed 16-bit PCM for editing and older devices.".into(),
            format_label: "WAV · 16-bit PCM".into(),
            extension: "wav".into(),
            video: None,
            audio: Some(audio(AudioCodec::Pcm, None, &[], 44100, 2)),
            requires: vec!["pcm_s16le".into()],
            available: true,
            unavailable_reason: None,
            original: false,
        },
    ];
    for p in &mut list {
        if let Some(a) = &mut p.audio {
            if !a.sample_rates.contains(&a.sample_rate) {
                a.sample_rates.push(a.sample_rate);
                a.sample_rates.sort_unstable();
            }
        }
    }
    list
}

/// Catalog with availability resolved against the installed FFmpeg.
pub fn catalog_for(tools: &Tools) -> Vec<Preset> {
    let mut list = catalog();
    for p in &mut list {
        let missing: Vec<String> = p
            .requires
            .iter()
            .filter(|r| if r.as_str() == "h264" { tools.h264_encoder().is_none() } else { !tools.has_encoder(r) })
            .cloned()
            .collect();
        if !missing.is_empty() {
            p.available = false;
            p.unavailable_reason = Some(format!(
                "The bundled FFmpeg has no {} encoder.",
                missing.join(" / ")
            ));
        }
    }
    list
}

pub fn find(id: &str) -> Option<Preset> {
    catalog().into_iter().find(|p| p.id == id)
}
