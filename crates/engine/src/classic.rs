//! The "Classic player AVI" pipeline: FFmpeg decodes and scales, everything
//! else (JPEG, IMA ADPCM, AVI) is encoded in-process so the output matches the
//! original converter's format exactly.

use std::ffi::OsString;
use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::adpcm::ImaAdpcmEncoder;
use crate::avi::{ClassicAviWriter, ClassicParams, CLASSIC_SAMPLE_RATE, HEADER_LEN};
use crate::error::{Error, Result};
use crate::jpeg::JpegEncoder;
use crate::plan::{video_filter, Estimate, EstimateMethod, Plan, VideoPlan};
use crate::progress::{CancelToken, Progress, Tracker};
use crate::run::{collect_stderr, kill, summarize_error};
use crate::tools::Tools;

/// JPEG quality of the original encoder's tables.
pub const CLASSIC_QUALITY: u8 = 75;

fn os<S: Into<OsString>>(s: S) -> OsString {
    s.into()
}

fn input_args(plan: &Plan, input: &Path) -> Vec<OsString> {
    let mut a = vec![os("-nostdin"), os("-hide_banner"), os("-loglevel"), os("error")];
    if plan.start > 0.0 {
        a.extend([os("-ss"), os(format!("{:.3}", plan.start))]);
    }
    a.extend([os("-i"), input.as_os_str().to_owned()]);
    if plan.duration > 0.0 {
        a.extend([os("-t"), os(format!("{:.3}", plan.duration))]);
    }
    a
}

fn spawn_video(tools: &Tools, plan: &Plan, v: &VideoPlan, input: &Path) -> Result<Child> {
    let mut a = input_args(plan, input);
    a.extend([
        os("-map"),
        os("0:v:0"),
        os("-vf"),
        os(video_filter(v)),
        os("-pix_fmt"),
        os("rgb24"),
        os("-f"),
        os("rawvideo"),
        os("pipe:1"),
    ]);
    Ok(tools.ffmpeg_cmd().args(a).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?)
}

fn spawn_audio(tools: &Tools, plan: &Plan, input: &Path) -> Result<Child> {
    let mut a = input_args(plan, input);
    a.extend([
        os("-map"),
        os("0:a:0"),
        os("-vn"),
        os("-ac"),
        os("1"),
        os("-ar"),
        os(CLASSIC_SAMPLE_RATE.to_string()),
        os("-c:a"),
        os("pcm_s16le"),
        os("-f"),
        os("s16le"),
        os("pipe:1"),
    ]);
    Ok(tools.ffmpeg_cmd().args(a).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?)
}

fn read_exact_or_eof(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

fn audio_blocks(mut child_stdout: impl Read + Send + 'static, spb: usize) -> Receiver<Vec<i16>> {
    let (tx, rx) = mpsc::sync_channel::<Vec<i16>>(64);
    thread::spawn(move || {
        let mut buf = vec![0u8; spb * 2];
        loop {
            let n = match read_exact_or_eof(&mut child_stdout, &mut buf) {
                Ok(n) => n,
                Err(_) => break,
            };
            if n == 0 {
                break;
            }
            let mut block = vec![0i16; spb];
            for (i, s) in block.iter_mut().enumerate().take(n / 2) {
                *s = i16::from_le_bytes([buf[2 * i], buf[2 * i + 1]]);
            }
            if tx.send(block).is_err() || n < buf.len() {
                break;
            }
        }
    });
    rx
}

/// Convert `input` into a classic-player AVI at `output`.
pub fn convert(
    tools: &Tools,
    plan: &Plan,
    input: &Path,
    output: &Path,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<u64> {
    let v = plan.video.as_ref().ok_or(Error::MissingStream("video"))?;
    let spb = v.samples_per_block.unwrap_or(1017);
    let params = ClassicParams::new(v.width, v.height, spb);
    let frame_bytes = (v.width * v.height * 3) as usize;

    let mut video = spawn_video(tools, plan, v, input)?;
    let video_err = collect_stderr(&mut video);
    let video_out = video.stdout.take().expect("piped");
    let (ftx, frx) = mpsc::sync_channel::<Vec<u8>>(8);
    let frame_reader = thread::spawn(move || {
        let mut r = video_out;
        loop {
            let mut frame = vec![0u8; frame_bytes];
            match read_exact_or_eof(&mut r, &mut frame) {
                Ok(n) if n == frame_bytes => {
                    if ftx.send(frame).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    });

    let mut audio_child = None;
    let audio_rx = if plan.source_has_audio {
        let mut child = spawn_audio(tools, plan, input)?;
        let out = child.stdout.take().expect("piped");
        audio_child = Some(child);
        Some(audio_blocks(out, spb as usize))
    } else {
        None
    };

    let cleanup = |video: &mut Child, audio: &mut Option<Child>| {
        kill(video);
        if let Some(a) = audio.as_mut() {
            kill(a);
        }
    };

    let file = File::create(output)?;
    let mut writer = ClassicAviWriter::new(BufWriter::with_capacity(1 << 20, file), params)?;
    let mut jpeg = JpegEncoder::new(v.width, v.height, CLASSIC_QUALITY);
    let header = jpeg.header().to_vec();
    let mut adpcm = ImaAdpcmEncoder::new();
    let mut scan = Vec::with_capacity(frame_bytes / 4);
    let mut block = Vec::with_capacity(params.block_align() as usize);
    let silence = vec![0i16; spb as usize];
    let mut audio_done = audio_rx.is_none();
    let mut tracker = Tracker::new(plan.duration);
    let frame_secs = spb as f64 / CLASSIC_SAMPLE_RATE as f64;

    loop {
        if cancel.is_canceled() {
            cleanup(&mut video, &mut audio_child);
            drop(frx);
            let _ = frame_reader.join();
            return Err(Error::Canceled);
        }
        let frame = match frx.recv_timeout(Duration::from_millis(100)) {
            Ok(f) => f,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        scan.clear();
        jpeg.encode_scan(&frame, &mut scan);
        let samples = if audio_done {
            silence.clone()
        } else {
            match audio_rx.as_ref().map(|rx| rx.recv()) {
                Some(Ok(b)) => b,
                _ => {
                    audio_done = true;
                    silence.clone()
                }
            }
        };
        block.clear();
        adpcm.encode_block(&samples, &mut block);
        writer.write_frame(&header, &scan, &block)?;
        let done = writer.frames() as f64 * frame_secs;
        if let Some(p) = tracker.update(done.min(plan.duration.max(done)), writer.bytes_written()) {
            on_progress(p);
        }
    }

    let _ = frame_reader.join();
    let status = video.wait()?;
    let stderr = video_err.join().unwrap_or_default();
    if let Some(mut a) = audio_child {
        kill(&mut a);
    }
    if !status.success() {
        return Err(Error::Ffmpeg(summarize_error(&stderr, status.code())));
    }
    if writer.frames() == 0 {
        return Err(Error::Ffmpeg("No video frames could be decoded from this file.".into()));
    }
    let seconds = (writer.frames() as f64 * params.us_per_frame() as f64 / 1_000_000.0).round() as u32;
    let file = writer.finish(seconds)?;
    let len = file.get_ref().metadata()?.len();
    on_progress(tracker.finish(len));
    Ok(len)
}

/// Grab one RGB frame through a filter chain that ends at exactly `w` x `h`.
pub fn grab_rgb(tools: &Tools, input: &Path, at: f64, vf: &str, w: u32, h: u32) -> Result<Vec<u8>> {
    let out = tools
        .ffmpeg_cmd()
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-ss"])
        .arg(format!("{:.3}", at.max(0.0)))
        .arg("-i")
        .arg(input)
        .args(["-map", "0:v:0", "-frames:v", "1", "-vf", vf, "-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    let need = (w * h * 3) as usize;
    if out.stdout.len() < need {
        if at > 0.5 {
            // Seeking past the last keyframe can yield nothing; retry earlier.
            return grab_rgb(tools, input, (at - 1.0).max(0.0) * 0.5, vf, w, h);
        }
        return Err(Error::Ffmpeg(summarize_error(&String::from_utf8_lossy(&out.stderr), out.status.code())));
    }
    Ok(out.stdout[..need].to_vec())
}

/// Filter chain for a single frame (no fps conversion).
pub fn still_filter(v: &VideoPlan) -> String {
    let mut single = v.clone();
    single.fps_rational = None;
    video_filter(&single)
}

/// Estimate the classic AVI size by encoding real frames from the file.
pub fn estimate(tools: &Tools, plan: &Plan, input: &Path) -> Result<Estimate> {
    let v = plan.video.as_ref().ok_or(Error::MissingStream("video"))?;
    let spb = v.samples_per_block.unwrap_or(1017);
    let params = ClassicParams::new(v.width, v.height, spb);
    let frames = (plan.duration * v.fps).ceil().max(1.0);
    let samples = ((plan.duration / 2.0).ceil() as usize).clamp(1, 6);
    let vf = still_filter(v);
    let times: Vec<f64> =
        (0..samples).map(|i| plan.start + plan.duration * (i as f64 + 0.5) / samples as f64).collect();
    let sizes: Vec<Result<usize>> = thread::scope(|s| {
        let handles: Vec<_> = times
            .iter()
            .map(|&t| {
                let vf = vf.clone();
                s.spawn(move || -> Result<usize> {
                    let rgb = grab_rgb(tools, input, t, &vf, v.width, v.height)?;
                    let mut enc = JpegEncoder::new(v.width, v.height, CLASSIC_QUALITY);
                    let mut out = Vec::new();
                    enc.encode_scan(&rgb, &mut out);
                    Ok(out.len())
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(Error::Invalid("sampling failed".into())))).collect()
    });
    let ok: Vec<f64> = sizes.into_iter().filter_map(|r| r.ok()).map(|n| n as f64).collect();
    if ok.is_empty() {
        return Err(Error::Ffmpeg("Could not read frames to estimate the size.".into()));
    }
    let avg = ok.iter().sum::<f64>() / ok.len() as f64;
    let min = ok.iter().cloned().fold(f64::MAX, f64::min);
    let max = ok.iter().cloned().fold(0.0, f64::max);
    let fixed = params.fixed_bytes_per_frame() as f64 + 1.5;
    let base = HEADER_LEN as f64 + 8.0;
    let total = |scan: f64| base + frames * (fixed + scan);
    Ok(Estimate::range(total(avg), total(min * 0.9), total(max * 1.1), EstimateMethod::Sampled))
}
