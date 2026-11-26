//! Locating FFmpeg/FFprobe, detecting their capabilities, and spawning them.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub version: String,
    pub encoders: BTreeSet<String>,
    pub muxers: BTreeSet<String>,
}

fn exe_names(base: &str) -> Vec<String> {
    let mut v = Vec::new();
    if cfg!(windows) {
        v.push(format!("{base}.exe"));
    }
    v.push(base.to_string());
    // Android ships executables as lib<name>.so inside nativeLibraryDir.
    v.push(format!("lib{base}.so"));
    v
}

fn candidates(base: &str, env_var: &str, extra_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os(env_var) {
        out.push(PathBuf::from(p));
    }
    let mut dirs: Vec<PathBuf> = extra_dirs.to_vec();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            dirs.push(dir.join("ffmpeg"));
            dirs.push(dir.join("resources").join("ffmpeg"));
            dirs.push(dir.join("../lib/pocket-reel/ffmpeg"));
            dirs.push(dir.join("../lib/Pocket Reel/ffmpeg"));
        }
    }
    for d in &dirs {
        for n in exe_names(base) {
            out.push(d.join(n));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            for n in exe_names(base) {
                out.push(d.join(n));
            }
        }
    }
    out
}

/// Build a `Command` that never pops up a console window on Windows.
pub fn command<S: AsRef<OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(Stdio::null());
    cmd
}

fn works(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let out = command(path).arg("-hide_banner").arg("-version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    Some(text.lines().next().unwrap_or_default().to_string())
}

fn list(ffmpeg: &Path, what: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let Ok(out) = command(ffmpeg).args(["-hide_banner", what]).output() else {
        return set;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut started = false;
    for line in text.lines() {
        if line.trim_start().starts_with("--") {
            started = true;
            continue;
        }
        if !started {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(_flags), Some(name)) = (parts.next(), parts.next()) else { continue };
        for n in name.split(',') {
            set.insert(n.to_string());
        }
    }
    set
}

impl Tools {
    /// Find working ffmpeg and ffprobe binaries. `extra_dirs` are searched first
    /// (the app's bundled resource dir, Android's native library dir, ...).
    pub fn locate(extra_dirs: &[PathBuf]) -> Result<Tools> {
        let mut ffmpeg = None;
        for c in candidates("ffmpeg", "POCKETREEL_FFMPEG", extra_dirs) {
            if let Some(v) = works(&c) {
                ffmpeg = Some((c, v));
                break;
            }
        }
        let (ffmpeg, version) = ffmpeg.ok_or(Error::ToolMissing("ffmpeg"))?;
        let mut ffprobe = None;
        // Prefer the ffprobe that sits next to the chosen ffmpeg.
        let mut probe_dirs = Vec::new();
        if let Some(d) = ffmpeg.parent() {
            probe_dirs.push(d.to_path_buf());
        }
        probe_dirs.extend_from_slice(extra_dirs);
        for c in candidates("ffprobe", "POCKETREEL_FFPROBE", &probe_dirs) {
            if works(&c).is_some() {
                ffprobe = Some(c);
                break;
            }
        }
        let ffprobe = ffprobe.ok_or(Error::ToolMissing("ffprobe"))?;
        let encoders = list(&ffmpeg, "-encoders");
        let muxers = list(&ffmpeg, "-muxers");
        Ok(Tools { ffmpeg, ffprobe, version, encoders, muxers })
    }

    pub fn has_encoder(&self, name: &str) -> bool {
        self.encoders.contains(name)
    }

    pub fn has_muxer(&self, name: &str) -> bool {
        self.muxers.contains(name)
    }

    /// Best available H.264 encoder.
    pub fn h264_encoder(&self) -> Option<&'static str> {
        ["libx264", "libopenh264", "h264_mediacodec"].into_iter().find(|e| self.has_encoder(e))
    }

    pub fn ffmpeg_cmd(&self) -> Command {
        command(&self.ffmpeg)
    }

    pub fn ffprobe_cmd(&self) -> Command {
        command(&self.ffprobe)
    }
}
