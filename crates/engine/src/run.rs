//! Running FFmpeg with live progress and cancellation.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::progress::{parse_progress_line, CancelToken, Progress, Tracker};
use crate::tools::Tools;

/// Collect stderr in the background, keeping the tail.
pub(crate) fn collect_stderr(child: &mut Child) -> thread::JoinHandle<String> {
    let stderr = child.stderr.take();
    thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut e) = stderr {
            let mut buf = Vec::new();
            let _ = e.read_to_end(&mut buf);
            text = String::from_utf8_lossy(&buf).to_string();
        }
        if text.len() > 4000 {
            let cut = text.len() - 4000;
            let cut = (cut..text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(cut);
            text = text[cut..].to_string();
        }
        text
    })
}

/// Turn FFmpeg's stderr into a short human message.
pub(crate) fn summarize_error(stderr: &str, code: Option<i32>) -> String {
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let pick = lines
        .iter()
        .rev()
        .find(|l| !l.starts_with("Conversion failed") && !l.contains("Terminating thread"))
        .or(lines.last());
    match pick {
        Some(l) => {
            // Strip "[component @ 0x...] " prefixes.
            let l = match (l.starts_with('['), l.find("] ")) {
                (true, Some(i)) => &l[i + 2..],
                _ => l,
            };
            l.to_string()
        }
        None => format!("FFmpeg exited with code {}", code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())),
    }
}

pub(crate) fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Run FFmpeg with `-progress pipe:1` arguments. `total` is the output media
/// duration in seconds, used for the fraction and ETA.
pub fn run_ffmpeg(
    tools: &Tools,
    args: &[OsString],
    total: f64,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let mut child = tools
        .ffmpeg_cmd()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("stdout piped");
    let err_handle = collect_stderr(&mut child);
    let (tx, rx) = mpsc::channel::<(f64, u64)>();
    let reader = thread::spawn(move || {
        let mut media = 0.0;
        let mut bytes = 0u64;
        for line in BufReader::new(stdout).lines().map_while(std::io::Result::ok) {
            if let Some((k, v)) = parse_progress_line(&line) {
                match k {
                    "out_time_us" | "out_time_ms" => {
                        if let Ok(us) = v.parse::<i64>() {
                            media = (us.max(0) as f64) / 1_000_000.0;
                        }
                    }
                    "total_size" => {
                        if let Ok(b) = v.parse::<u64>() {
                            bytes = b;
                        }
                    }
                    "progress" => {
                        if tx.send((media, bytes)).is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    let mut tracker = Tracker::new(total);
    let mut last_bytes = 0;
    let status = loop {
        if cancel.is_canceled() {
            kill(&mut child);
            let _ = reader.join();
            let _ = err_handle.join();
            return Err(Error::Canceled);
        }
        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok((media, bytes)) => {
                last_bytes = bytes;
                if let Some(p) = tracker.update(media.min(total.max(media)), bytes) {
                    on_progress(p);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // stdout closed: the process is exiting.
                break child.wait()?;
            }
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
    };
    let _ = reader.join();
    let stderr = err_handle.join().unwrap_or_default();
    if !status.success() {
        return Err(Error::Ffmpeg(summarize_error(&stderr, status.code())));
    }
    on_progress(tracker.finish(last_bytes));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_summary_strips_prefix() {
        let s = "[libmp3lame @ 0x55] Invalid bitrate\nConversion failed!\n";
        assert_eq!(summarize_error(s, Some(1)), "Invalid bitrate");
    }
}
