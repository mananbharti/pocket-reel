//! Downloading videos from links (YouTube and the other sites yt-dlp supports)
//! before converting them.
//!
//! Desktop builds run the bundled yt-dlp binary. Android supplies its own
//! [`Downloader`] backed by youtubedl-android.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::progress::CancelToken;
use crate::run::{collect_stderr, kill};
use crate::tools::command;

/// Download quality used when none is chosen: plenty for a 160 x 120 player
/// screen, and quick to download.
pub const DEFAULT_MAX_HEIGHT: u32 = 360;

/// yt-dlp format selection for a download no taller than `max_height`
/// (`None` = the best available). Prefers H.264 + AAC, which is cheap to decode,
/// then any video and audio up to that height, then whatever exists.
pub fn format_for(max_height: Option<u32>) -> String {
    match max_height {
        Some(h) => format!("bv*[height<={h}][vcodec^=avc1]+ba[ext=m4a]/bv*[height<={h}]+ba/b[height<={h}]/wv*+ba/w"),
        None => "bv*+ba/b".to_string(),
    }
}

/// One download progress update.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DownloadProgress {
    /// 0.0 ..= 1.0 across all streams of the download.
    pub fraction: f64,
    /// Bytes downloaded so far (all streams).
    pub downloaded: u64,
    /// Expected size of the whole download, when known.
    pub total: Option<u64>,
    /// Bytes per second.
    pub speed: Option<f64>,
    /// Seconds left, when known.
    pub eta: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub url: String,
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
}

/// Something that can fetch a link's details and download it.
pub trait Downloader: Send + Sync {
    fn info(&self, url: &str) -> Result<RemoteInfo>;

    /// Download `url` into `dir` no taller than `max_height` (`None` = best) and
    /// return the file.
    fn download(
        &self,
        url: &str,
        dir: &Path,
        max_height: Option<u32>,
        cancel: &CancelToken,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> Result<PathBuf>;
}

/// True for text that is a single http(s) link.
pub fn is_url(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with("https://") || t.starts_with("http://")) && !t.contains(char::is_whitespace) && t.len() > 10
}

/// Find the first http(s) link in shared text such as
/// "Check this out https://youtu.be/abc?si=x".
pub fn extract_url(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '"' || c == '<' || c == '>')
        .map(|w| w.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | ',' | '\'')))
        .find(|w| is_url(w))
        .map(str::to_string)
}

/// Make a title safe as a file name on every platform.
pub fn safe_file_stem(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { ' ' } else { c })
        .collect();
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    let mut out: String = s.chars().take(120).collect();
    if out.is_empty() {
        out = "video".into();
    }
    out
}

/// Turn yt-dlp's error output into one helpful sentence.
pub fn friendly_error(stderr: &str) -> String {
    let l = stderr.to_ascii_lowercase();
    let last = stderr
        .lines()
        .rev()
        .find(|line| line.contains("ERROR"))
        .map(|line| line.trim_start_matches("ERROR:").trim().to_string());
    if l.contains("not a bot") || l.contains("sign in to confirm") {
        "YouTube asked to confirm you're not a bot. Wait a while and try again.".into()
    } else if l.contains("private video") {
        "This video is private.".into()
    } else if l.contains("video unavailable") || l.contains("this video is not available") {
        "This video isn't available.".into()
    } else if l.contains("age") && l.contains("restrict") {
        "This video is age-restricted and can't be downloaded without signing in.".into()
    } else if l.contains("unsupported url") {
        "This link isn't a video Pocket Reel can download.".into()
    } else if l.contains("unable to download webpage") || l.contains("getaddrinfo") || l.contains("network is unreachable") || l.contains("timed out") {
        "Couldn't reach the website. Check your internet connection.".into()
    } else {
        last.map(|m| {
            // Strip "[youtube] abc123: " prefixes.
            match m.find(']') {
                Some(i) if m.starts_with('[') => {
                    let rest = m[i + 1..].trim();
                    rest.split_once(": ").map(|(_, msg)| msg).unwrap_or(rest).trim().to_string()
                }
                _ => m,
            }
        })
        .or_else(|| {
            // Not a yt-dlp error line (for example the Android library failing to
            // start): show its last line rather than hiding it.
            stderr.lines().rev().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(300).collect())
        })
        .unwrap_or_else(|| "The download failed.".into())
    }
}

/// Parse yt-dlp's `--dump-single-json` output.
pub fn parse_info(url: &str, json: &str) -> Result<RemoteInfo> {
    let v: Value = serde_json::from_str(json).map_err(|e| Error::Invalid(format!("Unexpected answer from yt-dlp: {e}")))?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Ok(RemoteInfo {
        url: s("webpage_url").unwrap_or_else(|| url.to_string()),
        id: s("id").unwrap_or_default(),
        title: s("title").filter(|t| !t.is_empty()).unwrap_or_else(|| "Video".into()),
        duration: v.get("duration").and_then(Value::as_f64).filter(|d| *d > 0.0),
        thumbnail: s("thumbnail"),
        uploader: s("uploader").or_else(|| s("channel")),
    })
}

/// Combine per-stream download progress into one 0..1 value. Video downloads
/// first and is most of the data; a separate audio stream follows; merging is quick.
#[derive(Default)]
pub struct StreamProgress {
    stream: usize,
    last_downloaded: u64,
    last_done: bool,
    /// Bytes of the streams that already finished.
    finished_bytes: u64,
    last_total: Option<u64>,
}

impl StreamProgress {
    /// Feed one stream's progress; returns the combined progress.
    pub fn update(&mut self, downloaded: u64, total: Option<u64>, speed: Option<f64>, eta: Option<f64>) -> DownloadProgress {
        // A new stream starts when the byte count goes back down after one finished
        // (or drops a lot, if the finished line was missed).
        if downloaded < self.last_downloaded && (self.last_done || downloaded + 1024 < self.last_downloaded) {
            self.stream += 1;
            self.finished_bytes += self.last_total.unwrap_or(self.last_downloaded).max(self.last_downloaded);
        }
        self.last_downloaded = downloaded;
        self.last_total = total;
        self.last_done = total.is_some_and(|t| downloaded >= t);
        let f = total.map(|t| (downloaded as f64 / t.max(1) as f64).clamp(0.0, 1.0));
        let fraction = match (self.stream, f) {
            (0, Some(f)) => f * 0.82,
            (_, Some(f)) => 0.82 + f * 0.16,
            (0, None) => 0.0,
            (_, None) => 0.82,
        };
        DownloadProgress {
            fraction,
            downloaded: self.finished_bytes + downloaded,
            total: total.map(|t| self.finished_bytes + t),
            speed: speed.filter(|v| *v > 0.0),
            eta: eta.filter(|v| *v >= 0.0),
        }
    }
}

/// Parse a yt-dlp progress line such as
/// `[download]  45.3% of ~  10.00MiB at    1.20MiB/s ETA 00:07 (frag 3/9)`
/// into (percent, total bytes, bytes per second, seconds left).
pub fn parse_progress_line(line: &str) -> Option<(f64, Option<u64>, Option<f64>, Option<f64>)> {
    let rest = line.trim().strip_prefix("[download]")?.trim();
    let (pct, rest) = rest.split_once('%')?;
    let pct: f64 = pct.trim().parse().ok()?;
    let words: Vec<&str> = rest.split_whitespace().collect();
    let after = |key: &str| {
        let i = words.iter().position(|w| *w == key)?;
        words[i + 1..].iter().find(|w| **w != "~").copied()
    };
    let total = after("of").and_then(parse_size).map(|b| b.round() as u64);
    let speed = after("at").and_then(|v| parse_size(v.trim_end_matches("/s")));
    let eta = after("ETA").and_then(|t| {
        t.split(':').try_fold(0.0, |acc, part| part.parse::<f64>().ok().map(|n| acc * 60.0 + n))
    });
    Some((pct, total, speed, eta))
}

/// "10.00MiB" -> bytes.
fn parse_size(s: &str) -> Option<f64> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (n, unit) = s.split_at(split);
    let n: f64 = n.parse().ok()?;
    let mult = match unit {
        "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "KB" | "kB" => 1000.0,
        "MB" => 1e6,
        "GB" => 1e9,
        _ => return None,
    };
    Some(n * mult)
}

/// The yt-dlp command-line program.
#[derive(Debug, Clone)]
pub struct YtDlp {
    pub bin: PathBuf,
    pub ffmpeg: PathBuf,
    /// Value for `--js-runtimes`, for example `quickjs:/path/qjs`.
    pub js_runtime: Option<String>,
}

fn exe_names(base: &str) -> Vec<String> {
    let mut v = Vec::new();
    if cfg!(windows) {
        v.push(format!("{base}.exe"));
    }
    v.push(base.to_string());
    v
}

fn find_in(dirs: &[PathBuf], names: &[String]) -> Option<PathBuf> {
    for d in dirs {
        for n in names {
            let p = d.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

impl YtDlp {
    /// Find yt-dlp (and QuickJS) next to the bundled FFmpeg, in `extra_dirs`, or on PATH.
    pub fn locate(extra_dirs: &[PathBuf], ffmpeg: &Path) -> Option<YtDlp> {
        let mut dirs: Vec<PathBuf> = extra_dirs.to_vec();
        if let Some(d) = ffmpeg.parent() {
            dirs.push(d.to_path_buf());
        }
        if let Some(path) = std::env::var_os("PATH") {
            dirs.extend(std::env::split_paths(&path));
        }
        let bin = std::env::var_os("POCKETREEL_YTDLP").map(PathBuf::from).filter(|p| p.is_file()).or_else(|| {
            let mut names = exe_names("yt-dlp");
            names.push("yt-dlp_linux".into());
            names.push("yt-dlp_macos".into());
            find_in(&dirs, &names)
        })?;
        let js_runtime = std::env::var("POCKETREEL_JS_RUNTIME").ok().or_else(|| {
            find_in(&dirs, &exe_names("qjs")).map(|p| format!("quickjs:{}", p.display()))
        });
        Some(YtDlp { bin, ffmpeg: ffmpeg.to_path_buf(), js_runtime })
    }

    fn base_args(&self) -> Vec<String> {
        let mut a = vec!["--no-playlist".to_string(), "--no-config".into(), "--ffmpeg-location".into(), self.ffmpeg.display().to_string()];
        if let Some(js) = &self.js_runtime {
            a.extend(["--js-runtimes".into(), js.clone()]);
        }
        a
    }

    pub fn version(&self) -> Option<String> {
        let out = command(&self.bin).arg("--version").output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Keep a writable copy in `data_dir` so yt-dlp can update itself (YouTube
    /// changes often), and check for updates at most once a day in the background.
    pub fn make_updatable(&mut self, data_dir: &Path) {
        let name = self.bin.file_name().map(|n| n.to_owned()).unwrap_or_else(|| "yt-dlp".into());
        let dir = data_dir.join("yt-dlp");
        let copy = dir.join(&name);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let bundled = self.version();
        let current = YtDlp { bin: copy.clone(), ..self.clone() }.version();
        let needs_copy = match (&current, &bundled) {
            (None, _) => true,
            (Some(c), Some(b)) => b > c,
            _ => false,
        };
        if needs_copy && std::fs::copy(&self.bin, &copy).is_err() {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o755));
        }
        self.bin = copy.clone();
        let stamp = dir.join("last-update-check");
        let stale = std::fs::metadata(&stamp)
            .and_then(|m| m.modified())
            .map(|t| SystemTime::now().duration_since(t).unwrap_or_default() > Duration::from_secs(86_400))
            .unwrap_or(true);
        if stale {
            let bin = copy;
            thread::spawn(move || {
                let ok = command(&bin).args(["-U", "--no-config"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
                if ok.map(|s| s.success()).unwrap_or(false) {
                    let _ = std::fs::write(&stamp, b"ok");
                }
            });
        }
    }
}

impl Downloader for YtDlp {
    fn info(&self, url: &str) -> Result<RemoteInfo> {
        let out = command(&self.bin)
            .args(self.base_args())
            .args(["--skip-download", "--dump-single-json", "--no-warnings", "--"])
            .arg(url)
            .output()?;
        if !out.status.success() {
            return Err(Error::Download(friendly_error(&String::from_utf8_lossy(&out.stderr))));
        }
        parse_info(url, &String::from_utf8_lossy(&out.stdout))
    }

    fn download(
        &self,
        url: &str,
        dir: &Path,
        max_height: Option<u32>,
        cancel: &CancelToken,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let template = dir.join("%(id)s.%(ext)s");
        let format = format_for(max_height);
        let mut child = command(&self.bin)
            .args(self.base_args())
            .args([
                "-f",
                &format,
                "--merge-output-format",
                "mp4",
                "--no-mtime",
                "--no-part",
                "--newline",
                "--progress",
                "--progress-template",
                "download:PRG %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.eta)s %(progress.speed)s",
                "--print",
                "after_move:FILE %(filepath)s",
                "-o",
            ])
            .arg(&template)
            .arg("--")
            .arg(url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().expect("piped");
        let err = collect_stderr(&mut child);
        let (tx, rx) = mpsc::channel::<String>();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(std::io::Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut streams = StreamProgress::default();
        let mut file: Option<PathBuf> = None;
        let num = |s: Option<&str>| s.and_then(|v| v.parse::<f64>().ok());
        let status = loop {
            if cancel.is_canceled() {
                kill(&mut child);
                let _ = reader.join();
                let _ = err.join();
                return Err(Error::Canceled);
            }
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => {
                    if let Some(rest) = line.strip_prefix("PRG ") {
                        let mut it = rest.split_whitespace();
                        let done = num(it.next()).unwrap_or(0.0) as u64;
                        let exact = num(it.next());
                        let estimate = num(it.next());
                        let total = exact.or(estimate).map(|t| t as u64);
                        let eta = num(it.next());
                        let speed = num(it.next());
                        on_progress(streams.update(done, total, speed, eta));
                    } else if let Some(p) = line.strip_prefix("FILE ") {
                        file = Some(PathBuf::from(p.trim()));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break child.wait()?,
            }
        };
        let _ = reader.join();
        let stderr = err.join().unwrap_or_default();
        if !status.success() {
            return Err(Error::Download(friendly_error(&stderr)));
        }
        let file = file.filter(|f| f.is_file()).or_else(|| {
            // Fall back to the newest media file in the folder.
            std::fs::read_dir(dir)
                .ok()?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        });
        let file = file.ok_or_else(|| Error::Download("The download finished but no video file was found.".into()))?;
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        on_progress(DownloadProgress { fraction: 1.0, downloaded: size, total: Some(size), speed: None, eta: Some(0.0) });
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_links_in_shared_text() {
        assert_eq!(
            extract_url("Watch this! https://youtu.be/aqz-KE-bpKQ?si=abc shared via YouTube").as_deref(),
            Some("https://youtu.be/aqz-KE-bpKQ?si=abc")
        );
        assert_eq!(extract_url("no link here"), None);
        assert!(is_url("https://www.youtube.com/watch?v=x1"));
        assert!(!is_url("hello https://a.b"));
    }

    #[test]
    fn safe_names() {
        assert_eq!(safe_file_stem("A/B: C? \"D\"  "), "A B C D");
        assert_eq!(safe_file_stem("..."), "video");
    }

    #[test]
    fn friendly_messages() {
        assert!(friendly_error("ERROR: [youtube] x: Sign in to confirm you’re not a bot").contains("not a bot"));
        assert_eq!(friendly_error("ERROR: [youtube] abc: Some odd failure"), "Some odd failure");
        assert_eq!(friendly_error("failed to initialize: libpython.so missing"), "failed to initialize: libpython.so missing");
        assert_eq!(friendly_error(""), "The download failed.");
    }

    #[test]
    fn stream_progress_combines_video_and_audio() {
        let mut s = StreamProgress::default();
        assert!((s.update(50, Some(100), None, None).fraction - 0.41).abs() < 1e-9);
        assert!((s.update(100, Some(100), None, None).fraction - 0.82).abs() < 1e-9);
        let a = s.update(10, Some(40), Some(5.0), Some(3.0));
        assert!(a.fraction > 0.82 && a.fraction < 0.9);
        assert_eq!(a.downloaded, 110);
        assert_eq!(a.total, Some(140));
        assert_eq!(a.speed, Some(5.0));
    }

    #[test]
    fn parses_progress_lines() {
        let (p, total, speed, eta) =
            parse_progress_line("[download]  45.3% of ~  10.00MiB at    1.50MiB/s ETA 01:07 (frag 3/9)").unwrap();
        assert_eq!(p, 45.3);
        assert_eq!(total, Some(10 * 1024 * 1024));
        assert_eq!(speed, Some(1.5 * 1024.0 * 1024.0));
        assert_eq!(eta, Some(67.0));
        let (p, total, speed, eta) = parse_progress_line("[download] 100% of    3.20KiB in 00:00:01 at 2.00KiB/s").unwrap();
        assert_eq!(p, 100.0);
        assert_eq!(total, Some(3277));
        assert_eq!(speed, Some(2048.0));
        assert_eq!(eta, None);
        assert!(parse_progress_line("[info] Downloading format 18").is_none());
    }

    #[test]
    fn formats_by_quality() {
        assert!(format_for(Some(360)).contains("height<=360"));
        assert!(format_for(Some(720)).contains("height<=720"));
        assert_eq!(format_for(None), "bv*+ba/b");
    }

    #[test]
    fn parses_info_json() {
        let i = parse_info("u", r#"{"id":"x","title":"Big Buck Bunny","duration":596,"thumbnail":"https://i.ytimg.com/x.jpg","uploader":"Blender"}"#).unwrap();
        assert_eq!(i.title, "Big Buck Bunny");
        assert_eq!(i.duration, Some(596.0));
    }
}
