//! `pocketreel` command-line tool.

mod serve;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;

use clap::{Parser, Subcommand, ValueEnum};
use pocketreel_engine::{
    format_bytes, plan, presets, preview, Engine, Event, Fit, JobRequest, JobState, Settings, Tools,
};

#[derive(Parser)]
#[command(name = "pocketreel", version, about = "Convert videos for any player. MP4 to classic AVI, MP3, MP4, WebM and more.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum FitArg {
    Stretch,
    Letterbox,
    Crop,
    Contain,
}

impl From<FitArg> for Fit {
    fn from(f: FitArg) -> Fit {
        match f {
            FitArg::Stretch => Fit::Stretch,
            FitArg::Letterbox => Fit::Letterbox,
            FitArg::Crop => Fit::Crop,
            FitArg::Contain => Fit::Contain,
        }
    }
}

#[derive(clap::Args, Clone)]
struct Opts {
    /// Preset id (see `pocketreel presets`).
    #[arg(short, long, default_value = "classic-avi")]
    preset: String,
    /// Output frame size, e.g. 160x120.
    #[arg(long, value_parser = parse_size)]
    size: Option<(u32, u32)>,
    #[arg(long, value_enum)]
    fit: Option<FitArg>,
    /// Frames per second (0 keeps the source rate).
    #[arg(long)]
    fps: Option<f64>,
    /// Video bitrate in kbps.
    #[arg(long)]
    vbitrate: Option<u32>,
    /// Audio bitrate in kbps.
    #[arg(long)]
    abitrate: Option<u32>,
    /// Audio sample rate in Hz.
    #[arg(long)]
    rate: Option<u32>,
    /// Audio channels (1 or 2).
    #[arg(long)]
    channels: Option<u32>,
    /// Trim start in seconds.
    #[arg(long)]
    start: Option<f64>,
    /// Trim end in seconds.
    #[arg(long)]
    end: Option<f64>,
}

impl Opts {
    fn settings(&self) -> Settings {
        Settings {
            preset: self.preset.clone(),
            width: self.size.map(|s| s.0),
            height: self.size.map(|s| s.1),
            fit: self.fit.map(Into::into),
            fps: self.fps,
            video_bitrate_kbps: self.vbitrate,
            audio_bitrate_kbps: self.abitrate,
            sample_rate: self.rate,
            channels: self.channels,
            trim_start: self.start,
            trim_end: self.end,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// List presets.
    Presets {
        #[arg(long)]
        json: bool,
    },
    /// Show media information.
    Probe {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Show the output size and estimated file size without converting.
    Estimate {
        file: PathBuf,
        #[command(flatten)]
        opts: Opts,
        #[arg(long)]
        json: bool,
    },
    /// Save one output-framed frame as JPEG.
    Preview {
        file: PathBuf,
        #[command(flatten)]
        opts: Opts,
        /// Time in seconds.
        #[arg(long, default_value_t = 1.0)]
        at: f64,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Convert one or more files.
    Convert {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[command(flatten)]
        opts: Opts,
        /// Output folder (default: next to each input).
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
        /// Parallel jobs.
        #[arg(short, long, default_value_t = 1)]
        jobs: usize,
    },
    /// Serve the engine over HTTP for UI development.
    Serve {
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Folder with the built web UI to serve at /.
        #[arg(long)]
        ui: Option<PathBuf>,
        /// Folder for files uploaded from the browser.
        #[arg(long)]
        uploads: Option<PathBuf>,
    },
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once(['x', 'X', '*']).ok_or("use WIDTHxHEIGHT, e.g. 160x120")?;
    Ok((w.trim().parse().map_err(|_| "bad width")?, h.trim().parse().map_err(|_| "bad height")?))
}

fn tools() -> Result<Tools, String> {
    Tools::locate(&[]).map_err(|e| e.to_string())
}

fn fmt_time(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Presets { json } => {
            let t = tools()?;
            let list = presets::catalog_for(&t);
            if json {
                println!("{}", serde_json::to_string_pretty(&list).unwrap());
            } else {
                for p in list {
                    let mark = if p.available { " " } else { "!" };
                    println!("{mark} {:<12} {:<22} {}", p.id, p.name, p.format_label);
                }
            }
        }
        Cmd::Probe { file, json } => {
            let t = tools()?;
            let m = pocketreel_engine::probe::probe(&t, &file).map_err(|e| e.to_string())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&m).unwrap());
            } else {
                println!("{}  {}  {}", m.file_name, fmt_time(m.duration), format_bytes(m.size_bytes));
                if let Some(v) = &m.video {
                    println!("  video  {} {}x{} ({}x{} shown) {:.3} fps", v.codec, v.width, v.height, v.display_width, v.display_height, v.fps);
                }
                if let Some(a) = &m.audio {
                    println!("  audio  {} {} Hz, {} ch", a.codec, a.sample_rate, a.channels);
                }
            }
        }
        Cmd::Estimate { file, opts, json } => {
            let t = tools()?;
            let m = pocketreel_engine::probe::probe(&t, &file).map_err(|e| e.to_string())?;
            let p = plan::resolve(&t, &opts.settings(), &m).map_err(|e| e.to_string())?;
            let e = preview::estimate(&t, &p, &file).map_err(|e| e.to_string())?;
            if json {
                println!("{}", serde_json::json!({ "plan": p, "estimate": e }));
            } else {
                if let Some(v) = &p.video {
                    println!("Output   {} x {} at {:.2} fps ({:?})", v.width, v.height, v.fps, v.fit);
                }
                println!("Length   {}", fmt_time(p.duration));
                println!(
                    "Size     about {} ({} - {}, {:?})",
                    format_bytes(e.bytes),
                    format_bytes(e.low),
                    format_bytes(e.high),
                    e.method
                );
            }
        }
        Cmd::Preview { file, opts, at, output } => {
            let t = tools()?;
            let m = pocketreel_engine::probe::probe(&t, &file).map_err(|e| e.to_string())?;
            let p = plan::resolve(&t, &opts.settings(), &m).map_err(|e| e.to_string())?;
            let img = preview::output_preview(&t, &m, &p, at).map_err(|e| e.to_string())?;
            let b64 = img.data_url.split_once(',').map(|x| x.1).unwrap_or_default();
            let bytes = serve::b64decode(b64);
            std::fs::write(&output, bytes).map_err(|e| e.to_string())?;
            println!("{} x {} -> {}", img.width, img.height, output.display());
        }
        Cmd::Convert { files, opts, out, overwrite, jobs } => {
            let t = tools()?;
            let (tx, rx) = mpsc::channel();
            let engine = Engine::new(t, move |e| {
                let _ = tx.send(e);
            });
            engine.set_concurrency(jobs);
            let mut ids = Vec::new();
            for f in &files {
                let req = JobRequest {
                    input: f.to_string_lossy().to_string(),
                    settings: opts.settings(),
                    output_dir: out.as_ref().map(|o| o.to_string_lossy().to_string()),
                    overwrite,
                };
                match engine.enqueue(req) {
                    Ok(s) => ids.push(s.id),
                    Err(e) => eprintln!("skip {}: {e}", f.display()),
                }
            }
            let mut remaining = ids.len();
            let mut failed = 0;
            while remaining > 0 {
                let Ok(ev) = rx.recv() else { break };
                match ev {
                    Event::Progress { id, progress } => {
                        let eta = progress.eta_seconds.map(fmt_time).unwrap_or_else(|| "--:--".into());
                        eprint!(
                            "\r  job {id}: {:5.1}%  {:4.1}x  ETA {eta}   ",
                            progress.fraction * 100.0,
                            progress.speed
                        );
                        let _ = std::io::stderr().flush();
                    }
                    Event::Job { job } => match job.state {
                        JobState::Running => eprintln!("> {}  ->  {}", job.file_name, job.output),
                        JobState::Done => {
                            remaining -= 1;
                            eprintln!(
                                "\r  done: {} ({})                         ",
                                job.output,
                                format_bytes(job.output_bytes.unwrap_or(0))
                            );
                        }
                        JobState::Failed => {
                            remaining -= 1;
                            failed += 1;
                            eprintln!("\r  failed: {}                           ", job.error.unwrap_or_default());
                        }
                        JobState::Canceled => {
                            remaining -= 1;
                            failed += 1;
                        }
                        JobState::Queued => {}
                    },
                    Event::Removed { .. } => {}
                }
            }
            engine.shutdown();
            if failed > 0 {
                return Err(format!("{failed} conversion(s) failed"));
            }
        }
        Cmd::Serve { port, ui, uploads } => {
            let t = tools()?;
            serve::serve(t, port, ui, uploads)?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
