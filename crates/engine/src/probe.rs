//! Reading media information with ffprobe.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::tools::Tools;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    /// Size as displayed: sample aspect ratio and rotation applied.
    pub display_width: u32,
    pub display_height: u32,
    pub fps: f64,
    pub rotation: i32,
    pub bitrate: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub duration: f64,
    pub format: String,
    pub bitrate: Option<u64>,
    pub video: Option<VideoInfo>,
    pub audio: Option<AudioInfo>,
    pub title: Option<String>,
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn rational(s: &str) -> Option<f64> {
    let (a, b) = s.split_once('/')?;
    let (a, b): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
    if b == 0.0 || a == 0.0 {
        None
    } else {
        Some(a / b)
    }
}

fn rotation_of(stream: &Value) -> i32 {
    if let Some(list) = stream.get("side_data_list").and_then(Value::as_array) {
        for sd in list {
            if let Some(r) = sd.get("rotation").and_then(num) {
                return r.round() as i32;
            }
        }
    }
    stream
        .get("tags")
        .and_then(|t| t.get("rotate"))
        .and_then(num)
        .map(|r| r.round() as i32)
        .unwrap_or(0)
}

/// Parse `ffprobe -show_format -show_streams -of json` output.
pub fn parse_ffprobe_json(path: &Path, json: &str, size_bytes: u64) -> Result<MediaInfo> {
    let v: Value = serde_json::from_str(json)
        .map_err(|e| Error::Probe { path: path.to_path_buf(), message: e.to_string() })?;
    let format = v.get("format").cloned().unwrap_or(Value::Null);
    let streams = v.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut video = None;
    let mut audio = None;
    let mut stream_duration: f64 = 0.0;
    for s in &streams {
        let kind = s.get("codec_type").and_then(Value::as_str).unwrap_or("");
        let disposition_pic = s
            .get("disposition")
            .and_then(|d| d.get("attached_pic"))
            .and_then(num)
            .unwrap_or(0.0)
            > 0.0;
        if let Some(d) = s.get("duration").and_then(num) {
            stream_duration = stream_duration.max(d);
        }
        match kind {
            "video" if video.is_none() && !disposition_pic => {
                let width = s.get("width").and_then(num).unwrap_or(0.0) as u32;
                let height = s.get("height").and_then(num).unwrap_or(0.0) as u32;
                if width == 0 || height == 0 {
                    continue;
                }
                let sar = s
                    .get("sample_aspect_ratio")
                    .and_then(Value::as_str)
                    .and_then(|r| rational(&r.replace(':', "/")))
                    .unwrap_or(1.0);
                let rotation = rotation_of(s);
                let mut dw = ((width as f64 * sar).round() as u32).max(1);
                let mut dh = height;
                if rotation.rem_euclid(180) == 90 {
                    std::mem::swap(&mut dw, &mut dh);
                }
                let fps = s
                    .get("avg_frame_rate")
                    .and_then(Value::as_str)
                    .and_then(rational)
                    .filter(|f| *f > 0.0 && *f < 1000.0)
                    .or_else(|| s.get("r_frame_rate").and_then(Value::as_str).and_then(rational))
                    .unwrap_or(0.0);
                video = Some(VideoInfo {
                    codec: s.get("codec_name").and_then(Value::as_str).unwrap_or("unknown").to_string(),
                    width,
                    height,
                    display_width: dw,
                    display_height: dh,
                    fps,
                    rotation,
                    bitrate: s.get("bit_rate").and_then(num).map(|b| b as u64),
                });
            }
            "audio" if audio.is_none() => {
                audio = Some(AudioInfo {
                    codec: s.get("codec_name").and_then(Value::as_str).unwrap_or("unknown").to_string(),
                    sample_rate: s.get("sample_rate").and_then(num).unwrap_or(0.0) as u32,
                    channels: s.get("channels").and_then(num).unwrap_or(0.0) as u32,
                    bitrate: s.get("bit_rate").and_then(num).map(|b| b as u64),
                });
            }
            _ => {}
        }
    }

    let duration = format.get("duration").and_then(num).filter(|d| *d > 0.0).unwrap_or(stream_duration);
    if video.is_none() && audio.is_none() {
        return Err(Error::Probe {
            path: path.to_path_buf(),
            message: "no audio or video streams were found".into(),
        });
    }
    let title = format.get("tags").and_then(|t| t.get("title")).and_then(Value::as_str).map(str::to_string);
    Ok(MediaInfo {
        path: path.to_string_lossy().to_string(),
        file_name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        size_bytes,
        duration,
        format: format.get("format_name").and_then(Value::as_str).unwrap_or("").to_string(),
        bitrate: format.get("bit_rate").and_then(num).map(|b| b as u64),
        video,
        audio,
        title,
    })
}

/// Probe a media file.
pub fn probe(tools: &Tools, path: &Path) -> Result<MediaInfo> {
    let meta = std::fs::metadata(path)
        .map_err(|e| Error::Probe { path: path.to_path_buf(), message: e.to_string() })?;
    if !meta.is_file() {
        return Err(Error::Probe { path: path.to_path_buf(), message: "not a file".into() });
    }
    let out = tools
        .ffprobe_cmd()
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(path)
        .output()?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(Error::Probe {
            path: path.to_path_buf(),
            message: if msg.is_empty() { "unsupported or damaged file".into() } else { msg },
        });
    }
    parse_ffprobe_json(path, &String::from_utf8_lossy(&out.stdout), meta.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rotated_phone_video() {
        let json = r#"{"streams":[
          {"codec_type":"video","codec_name":"h264","width":1920,"height":1080,
           "avg_frame_rate":"30000/1001","sample_aspect_ratio":"1:1",
           "side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]},
          {"codec_type":"audio","codec_name":"aac","sample_rate":"48000","channels":2}],
          "format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2","duration":"12.5","bit_rate":"5000000"}}"#;
        let m = parse_ffprobe_json(Path::new("/x/clip.mp4"), json, 100).unwrap();
        let v = m.video.unwrap();
        assert_eq!((v.display_width, v.display_height), (1080, 1920));
        assert!((v.fps - 29.97).abs() < 0.01);
        assert_eq!(m.audio.unwrap().sample_rate, 48000);
        assert_eq!(m.duration, 12.5);
        assert_eq!(m.file_name, "clip.mp4");
    }

    #[test]
    fn ignores_cover_art_streams() {
        let json = r#"{"streams":[
          {"codec_type":"audio","codec_name":"mp3","sample_rate":"44100","channels":2},
          {"codec_type":"video","codec_name":"mjpeg","width":500,"height":500,"disposition":{"attached_pic":1}}],
          "format":{"format_name":"mp3","duration":"200.0"}}"#;
        let m = parse_ffprobe_json(Path::new("song.mp3"), json, 1).unwrap();
        assert!(m.video.is_none());
    }
}
