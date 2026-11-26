//! Job queue: batch conversion, cancellation and events.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::classic;
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
    pub plan: Plan,
    pub estimate: Option<Estimate>,
    pub duration: f64,
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

struct Inner {
    tools: Tools,
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
            plan,
            estimate: None,
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
        if output.exists() && !request.overwrite {
            let st = self.inner.state.lock().unwrap();
            let out_dir = request.output_dir.as_ref().map(PathBuf::from);
            output = Self::pick_output(&st, Path::new(&snap.input), out_dir.as_deref(), &snap.plan.extension, false);
        }
        let me = self.clone();
        let mut on_progress = move |p: Progress| {
            me.update(id, |s| s.progress = Some(p));
            me.emit(Event::Progress { id, progress: p });
        };
        let result = execute(&self.inner.tools, &snap.plan, Path::new(&snap.input), &output, request.overwrite, &cancel, &mut on_progress);
        let elapsed = started.elapsed().as_secs_f64();
        let final_snap = self.update(id, |s| {
            s.output = output.to_string_lossy().to_string();
            match &result {
                Ok(bytes) => {
                    s.state = JobState::Done;
                    s.output_bytes = Some(*bytes);
                    if let Some(p) = &mut s.progress {
                        p.fraction = 1.0;
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
