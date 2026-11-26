//! Job queue: batch conversion, cancellation and events.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::classic;
use crate::download::{safe_file_stem, Downloader, RemoteInfo};
use crate::error::{Error, Result};
use crate::plan::{self, ffmpeg_args, output_path, partial_path, Estimate, Plan, Settings};
use crate::probe::{self, MediaInfo};
use crate::progress::{CancelToken, Progress};
use crate::run::run_ffmpeg;
use crate::tools::Tools;

pub type JobId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    pub input: String,
    pub settings: Settings,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
    /// Set when `input` is a link to download first (YouTube and others).
    #[serde(default)]
    pub url: Option<String>,
    /// Title shown while downloading (from the link's details).
    #[serde(default)]
    pub title: Option<String>,
    /// Length in seconds, if known before downloading.
    #[serde(default)]
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub id: JobId,
    pub input: String,
    pub file_name: String,
    pub preset_id: String,
    pub preset_name: String,
    pub state: JobState,
    pub progress: Option<Progress>,
    pub output: String,
    pub output_bytes: Option<u64>,
    pub error: Option<String>,
    /// Known after probing; for links, after the download finished.
    pub plan: Option<Plan>,
    pub estimate: Option<Estimate>,
    pub duration: f64,
    /// The link this job downloads, if any.
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// A job was added or changed state.
    Job { job: JobSnapshot },
    /// Live progress for a running job.
    Progress { id: JobId, progress: Progress },
    /// A job was removed from the list.
    Removed { id: JobId },
}

type Sink = Box<dyn Fn(Event) + Send + Sync>;

struct Entry {
    snap: JobSnapshot,
    request: JobRequest,
    cancel: CancelToken,
}

struct State {
    jobs: Vec<Entry>,
    next_id: JobId,
    running: usize,
    concurrency: usize,
    shutdown: bool,
}

/// Where link downloads happen and where their results go by default.
struct DownloadSetup {
    downloader: Arc<dyn Downloader>,
    work_dir: PathBuf,
    output_dir: PathBuf,
}

struct Inner {
    tools: Tools,
    download: Mutex<Option<DownloadSetup>>,
    state: Mutex<State>,
    wake: Condvar,
    sink: Sink,
}

/// The conversion engine shared by the desktop app, the Android app and the CLI.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

impl Engine {
    pub fn new(tools: Tools, sink: impl Fn(Event) + Send + Sync + 'static) -> Engine {
        let inner = Arc::new(Inner {
            tools,
            download: Mutex::new(None),
            state: Mutex::new(State { jobs: Vec::new(), next_id: 1, running: 0, concurrency: 1, shutdown: false }),
            wake: Condvar::new(),
            sink: Box::new(sink),
        });
        let engine = Engine { inner };
        let dispatcher = engine.clone();
        thread::Builder::new()
            .name("pocketreel-dispatch".into())
            .spawn(move || dispatcher.dispatch_loop())
            .expect("spawn dispatcher");
        engine
    }

    pub fn tools(&self) -> &Tools {
        &self.inner.tools
    }

    /// Enable downloading from links. Downloads are kept in `work_dir` until
    /// converted; results go to `output_dir` unless a job names another folder.
    pub fn set_downloader(&self, downloader: Arc<dyn Downloader>, work_dir: PathBuf, output_dir: PathBuf) {
        *self.inner.download.lock().unwrap() = Some(DownloadSetup { downloader, work_dir, output_dir });
    }

    pub fn can_download(&self) -> bool {
        self.inner.download.lock().unwrap().is_some()
    }

    fn downloader(&self) -> Result<(Arc<dyn Downloader>, PathBuf, PathBuf)> {
        let d = self.inner.download.lock().unwrap();
        let d = d.as_ref().ok_or_else(|| Error::Invalid("Downloading from links isn't available in this build.".into()))?;
        Ok((d.downloader.clone(), d.work_dir.clone(), d.output_dir.clone()))
    }

    /// Details (title, length, thumbnail) for a link.
    pub fn link_info(&self, url: &str) -> Result<RemoteInfo> {
        let (d, _, _) = self.downloader()?;
        d.info(url)
    }

    pub fn probe(&self, path: &Path) -> Result<MediaInfo> {
        probe::probe(&self.inner.tools, path)
    }

    /// How many jobs may run at the same time (1..=4).
    pub fn set_concurrency(&self, n: usize) {
        let mut st = self.inner.state.lock().unwrap();
        st.concurrency = n.clamp(1, 4);
        self.inner.wake.notify_all();
    }

    pub fn jobs(&self) -> Vec<JobSnapshot> {
        self.inner.state.lock().unwrap().jobs.iter().map(|e| e.snap.clone()).collect()
    }

    fn emit(&self, e: Event) {
        (self.inner.sink)(e);
    }

    fn reserved_outputs(st: &State) -> HashSet<PathBuf> {
        st.jobs
            .iter()
            .filter(|e| matches!(e.snap.state, JobState::Queued | JobState::Running))
            .map(|e| PathBuf::from(&e.snap.output))
            .collect()
    }

    fn pick_output(st: &State, input: &Path, dir: Option<&Path>, ext: &str, overwrite: bool) -> PathBuf {
        let reserved = Self::reserved_outputs(st);
        let mut p = output_path(input, dir, ext, overwrite);
        let stem = input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut n = 2;
        while reserved.contains(&p) {
            p = p.with_file_name(format!("{stem} ({n}).{ext}"));
            while p.exists() && !overwrite {
                n += 1;
                p = p.with_file_name(format!("{stem} ({n}).{ext}"));
            }
            n += 1;
        }
        p
    }

    /// Validate and queue a conversion. Errors (unreadable file, invalid
    /// settings, missing encoder) are returned immediately.
    pub fn enqueue(&self, request: JobRequest) -> Result<JobSnapshot> {
        if request.url.is_some() {
            return self.enqueue_link(request);
        }
        let input = PathBuf::from(&request.input);
        let media = self.probe(&input)?;
        let plan = plan::resolve(&self.inner.tools, &request.settings, &media)?;
        let mut st = self.inner.state.lock().unwrap();
        let out_dir = request.output_dir.as_ref().map(PathBuf::from);
        let output = Self::pick_output(&st, &input, out_dir.as_deref(), &plan.extension, request.overwrite);
        let id = st.next_id;
        st.next_id += 1;
        let snap = JobSnapshot {
            id,
            input: request.input.clone(),
            file_name: media.file_name.clone(),
            preset_id: plan.preset_id.clone(),
            preset_name: plan.preset_name.clone(),
            state: JobState::Queued,
            progress: None,
            output: output.to_string_lossy().to_string(),
            output_bytes: None,
            error: None,
            duration: plan.duration,
            plan: Some(plan),
            estimate: None,
            url: None,
        };
        st.jobs.push(Entry { snap: snap.clone(), request, cancel: CancelToken::new() });
        drop(st);
        self.emit(Event::Job { job: snap.clone() });
        self.inner.wake.notify_all();
        Ok(snap)
    }

    fn enqueue_link(&self, request: JobRequest) -> Result<JobSnapshot> {
        let (_, _, default_out) = self.downloader()?;
        let preset = crate::presets::find(&request.settings.preset)
            .ok_or_else(|| Error::UnknownPreset(request.settings.preset.clone()))?;
        let title = request.title.clone().unwrap_or_else(|| "Video".into());
        let out_dir = request.output_dir.as_ref().map(PathBuf::from).unwrap_or(default_out);
        let mut st = self.inner.state.lock().unwrap();
        let fake_input = out_dir.join(format!("{}.download", safe_file_stem(&title)));
        let output = Self::pick_output(&st, &fake_input, Some(&out_dir), &preset.extension, request.overwrite);
        let id = st.next_id;
        st.next_id += 1;
        let snap = JobSnapshot {
            id,
            input: request.input.clone(),
            file_name: title,
            preset_id: preset.id.clone(),
            preset_name: preset.name.clone(),
            state: JobState::Queued,
            progress: None,
            output: output.to_string_lossy().to_string(),
            output_bytes: None,
            error: None,
            plan: None,
            estimate: None,
            duration: request.duration.unwrap_or(0.0),
            url: request.url.clone(),
        };
        st.jobs.push(Entry { snap: snap.clone(), request, cancel: CancelToken::new() });
        drop(st);
        self.emit(Event::Job { job: snap.clone() });
        self.inner.wake.notify_all();
        Ok(snap)
    }

    /// Cancel a queued or running job.
    pub fn cancel(&self, id: JobId) -> bool {
        let mut st = self.inner.state.lock().unwrap();
        let Some(e) = st.jobs.iter_mut().find(|e| e.snap.id == id) else { return false };
        match e.snap.state {
            JobState::Queued => {
                e.snap.state = JobState::Canceled;
                let snap = e.snap.clone();
                drop(st);
                self.emit(Event::Job { job: snap });
                true
            }
            JobState::Running => {
                e.cancel.cancel();
                true
            }
            _ => false,
        }
    }

    pub fn cancel_all(&self) {
        let ids: Vec<JobId> = self
            .jobs()
            .into_iter()
            .filter(|j| matches!(j.state, JobState::Queued | JobState::Running))
            .map(|j| j.id)
            .collect();
        for id in ids {
            self.cancel(id);
        }
    }

    /// Remove a finished, failed or canceled job from the list.
    pub fn remove(&self, id: JobId) -> bool {
        let mut st = self.inner.state.lock().unwrap();
        let before = st.jobs.len();
        st.jobs.retain(|e| !(e.snap.id == id && !matches!(e.snap.state, JobState::Running | JobState::Queued)));
        let removed = st.jobs.len() != before;
        drop(st);
        if removed {
            self.emit(Event::Removed { id });
        }
        removed
    }

    /// Put a failed or canceled job back in the queue.
    pub fn retry(&self, id: JobId) -> Result<JobSnapshot> {
        let request = {
            let st = self.inner.state.lock().unwrap();
            let e = st.jobs.iter().find(|e| e.snap.id == id).ok_or_else(|| Error::Invalid("No such job".into()))?;
            if !matches!(e.snap.state, JobState::Failed | JobState::Canceled) {
                return Err(Error::Invalid("Only failed or canceled jobs can be retried.".into()));
            }
            e.request.clone()
        };
        self.remove(id);
        self.enqueue(request)
    }

    pub fn shutdown(&self) {
        self.cancel_all();
        let mut st = self.inner.state.lock().unwrap();
        st.shutdown = true;
        self.inner.wake.notify_all();
    }

    fn dispatch_loop(&self) {
        loop {
            let next = {
                let mut st = self.inner.state.lock().unwrap();
                loop {
                    if st.shutdown {
                        return;
                    }
                    if st.running < st.concurrency {
                        if let Some(e) = st.jobs.iter_mut().find(|e| e.snap.state == JobState::Queued) {
                            e.snap.state = JobState::Running;
                            let job = (e.snap.clone(), e.request.clone(), e.cancel.clone());
                            st.running += 1;
                            break job;
                        }
                    }
                    st = self.inner.wake.wait(st).unwrap();
                }
            };
            self.emit(Event::Job { job: next.0.clone() });
            let me = self.clone();
            thread::Builder::new()
                .name(format!("pocketreel-job-{}", next.0.id))
                .spawn(move || me.run_job(next.0, next.1, next.2))
                .expect("spawn job");
        }
    }

    fn update<F: FnOnce(&mut JobSnapshot)>(&self, id: JobId, f: F) -> Option<JobSnapshot> {
        let mut st = self.inner.state.lock().unwrap();
        let e = st.jobs.iter_mut().find(|e| e.snap.id == id)?;
        f(&mut e.snap);
        Some(e.snap.clone())
    }

    fn run_job(self, snap: JobSnapshot, request: JobRequest, cancel: CancelToken) {
        let id = snap.id;
        let started = Instant::now();
        let mut output = PathBuf::from(&snap.output);
        let me = self.clone();
        let mut on_progress = move |p: Progress| {
            me.update(id, |s| s.progress = Some(p));
            me.emit(Event::Progress { id, progress: p });
        };

        let mut work_dir: Option<PathBuf> = None;
        let result = (|| -> Result<u64> {
            // 1. Links: download first.
            let (input, plan) = match &request.url {
                Some(url) => {
                    let (downloader, work, _) = self.downloader()?;
                    let dir = work.join(format!("job-{id}-{}", std::process::id()));
                    work_dir = Some(dir.clone());
                    let t0 = Instant::now();
                    let mut emit = |fraction: f64, eta: Option<f64>| {
                        on_progress(Progress {
                            fraction,
                            downloading: true,
                            media_seconds: 0.0,
                            speed: 0.0,
                            eta_seconds: eta,
                            elapsed_seconds: t0.elapsed().as_secs_f64(),
                            bytes: 0,
                        });
                    };
                    emit(0.0, None);
                    let file = downloader.download(url, &dir, &cancel, &mut emit)?;
                    let media = probe::probe(&self.inner.tools, &file)?;
                    let plan = plan::resolve(&self.inner.tools, &request.settings, &media)?;
                    self.update(id, |s| {
                        s.duration = plan.duration;
                        s.plan = Some(plan.clone());
                    });
                    (file, plan)
                }
                None => (
                    PathBuf::from(&snap.input),
                    snap.plan.clone().ok_or_else(|| Error::Invalid("Nothing to convert.".into()))?,
                ),
            };
            // 2. Convert.
            if output.exists() && !request.overwrite {
                let st = self.inner.state.lock().unwrap();
                let out_dir = output.parent().map(Path::to_path_buf);
                let named = match &request.url {
                    Some(_) => output.with_extension("download"),
                    None => PathBuf::from(&snap.input),
                };
                output = Self::pick_output(&st, &named, out_dir.as_deref(), &plan.extension, false);
            }
            execute(&self.inner.tools, &plan, &input, &output, request.overwrite, &cancel, &mut on_progress)
        })();
        if let Some(dir) = work_dir {
            let _ = fs::remove_dir_all(dir);
        }

        let elapsed = started.elapsed().as_secs_f64();
        let final_snap = self.update(id, |s| {
            s.output = output.to_string_lossy().to_string();
            match &result {
                Ok(bytes) => {
                    s.state = JobState::Done;
                    s.output_bytes = Some(*bytes);
                    if let Some(p) = &mut s.progress {
                        p.fraction = 1.0;
                        p.downloading = false;
                        p.eta_seconds = Some(0.0);
                        p.elapsed_seconds = elapsed;
                        p.bytes = *bytes;
                    }
                }
                Err(Error::Canceled) => s.state = JobState::Canceled,
                Err(e) => {
                    s.state = JobState::Failed;
                    s.error = Some(e.to_string());
                }
            }
        });
        {
            let mut st = self.inner.state.lock().unwrap();
            st.running -= 1;
        }
        self.inner.wake.notify_all();
        if let Some(s) = final_snap {
            self.emit(Event::Job { job: s });
        }
    }
}

/// Run one conversion synchronously, writing to a hidden temporary file and
/// renaming it into place only when everything succeeded.
pub fn execute(
    tools: &Tools,
    plan: &Plan,
    input: &Path,
    output: &Path,
    overwrite: bool,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<u64> {
    if let Some(dir) = output.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    let tmp = partial_path(output);
    let _ = fs::remove_file(&tmp);
    let result = if plan.classic {
        classic::convert(tools, plan, input, &tmp, cancel, on_progress)
    } else {
        run_ffmpeg(tools, &ffmpeg_args(plan, input, &tmp), plan.duration, cancel, on_progress)
            .and_then(|_| Ok(fs::metadata(&tmp)?.len()))
    };
    match result {
        Ok(bytes) => {
            if output.exists() {
                if overwrite {
                    fs::remove_file(output)?;
                } else {
                    let _ = fs::remove_file(&tmp);
                    return Err(Error::Invalid(format!("{} already exists.", output.display())));
                }
            }
            fs::rename(&tmp, output)?;
            Ok(bytes)
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}
