//! Tauri shell: exposes the engine to the web UI on desktop and Android.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use std::sync::Arc;

use pocketreel_engine::download::extract_url;
use pocketreel_engine::{
    plan, presets, preview, probe, Engine, Estimate, Event, JobRequest, JobSnapshot, MediaInfo, Plan, Preset,
    RemoteInfo, Settings, Tools,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

/// Registered before the window loads; the engine is filled in by a background
/// thread because finding and checking FFmpeg can take seconds on a phone.
struct AppState {
    engine: OnceLock<Result<Engine, String>>,
}

impl AppState {
    fn engine(&self) -> Result<Engine, String> {
        match self.engine.get() {
            Some(Ok(e)) => Ok(e.clone()),
            Some(Err(e)) => Err(e.clone()),
            None => Err("Pocket Reel is still starting. Try again in a moment.".into()),
        }
    }

    /// Wait (up to a minute) for startup to finish.
    fn wait(&self) -> Result<Engine, String> {
        for _ in 0..1200 {
            if self.engine.get().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.engine()
    }
}

async fn ready_engine(app: &AppHandle) -> Result<Engine, String> {
    let h = app.clone();
    tauri::async_runtime::spawn_blocking(move || h.state::<AppState>().wait())
        .await
        .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    platform: &'static str,
    ffmpeg: String,
    default_output_dir: Option<String>,
    version: String,
    engine_error: Option<String>,
    /// Pasting or sharing links works.
    downloads: bool,
}

fn platform() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else {
        "linux"
    }
}

/// Directories that may hold the bundled ffmpeg/ffprobe.
fn tool_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        dirs.push(res.join("ffmpeg"));
        dirs.push(res.join("resources").join("ffmpeg"));
        // A downloaded, unnotarised app keeps macOS's quarantine flag on the
        // bundled tools, which can stop them from running; clear it once.
        #[cfg(target_os = "macos")]
        {
            static ONCE: std::sync::Once = std::sync::Once::new();
            let dir = res.join("ffmpeg");
            ONCE.call_once(|| {
                let _ = std::process::Command::new("/usr/bin/xattr")
                    .args(["-dr", "com.apple.quarantine"])
                    .arg(&dir)
                    .status();
            });
        }
    }
    // Android packs the executables as lib*.so next to our own library.
    if cfg!(target_os = "android") {
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            for line in maps.lines() {
                if let Some(idx) = line.find('/') {
                    let p = Path::new(&line[idx..]);
                    if p.file_name().map(|n| n == "libpocket_reel_lib.so").unwrap_or(false) {
                        if let Some(d) = p.parent() {
                            dirs.push(d.to_path_buf());
                        }
                        break;
                    }
                }
            }
        }
    }
    dirs
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn app_info(app: AppHandle) -> Result<AppInfo, String> {
    let engine = ready_engine(&app).await;
    Ok(AppInfo {
        platform: platform(),
        ffmpeg: engine.as_ref().map(|e| e.tools().version.clone()).unwrap_or_default(),
        default_output_dir: android_output_dir(false).map(|p| p.to_string_lossy().to_string()),
        version: app.package_info().version.to_string(),
        downloads: engine.as_ref().map(|e| e.can_download()).unwrap_or(false),
        engine_error: engine.err(),
    })
}

#[tauri::command]
async fn presets(app: AppHandle) -> Result<Vec<Preset>, String> {
    Ok(presets::catalog_for(ready_engine(&app).await?.tools()))
}

#[tauri::command]
async fn probe(state: State<'_, AppState>, path: String) -> Result<MediaInfo, String> {
    let e = state.engine()?;
    blocking(move || e.probe(Path::new(&path)).map_err(|e| e.to_string())).await
}

#[tauri::command]
async fn thumbnail(
    state: State<'_, AppState>,
    path: String,
    at: Option<f64>,
    max_width: u32,
    max_height: u32,
) -> Result<preview::Image, String> {
    let e = state.engine()?;
    blocking(move || {
        let t = e.tools();
        let m = probe::probe(t, Path::new(&path)).map_err(|e| e.to_string())?;
        let at = at.unwrap_or((m.duration * 0.1).min(5.0));
        preview::thumbnail(t, &m, at, max_width, max_height).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn plan(state: State<'_, AppState>, path: String, settings: Settings) -> Result<Plan, String> {
    let e = state.engine()?;
    blocking(move || {
        let t = e.tools();
        let m = probe::probe(t, Path::new(&path)).map_err(|e| e.to_string())?;
        plan::resolve(t, &settings, &m).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn estimate(state: State<'_, AppState>, path: String, settings: Settings) -> Result<Estimate, String> {
    let e = state.engine()?;
    blocking(move || {
        let t = e.tools();
        let m = probe::probe(t, Path::new(&path)).map_err(|e| e.to_string())?;
        let p = plan::resolve(t, &settings, &m).map_err(|e| e.to_string())?;
        preview::estimate(t, &p, Path::new(&path)).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn preview(
    state: State<'_, AppState>,
    path: String,
    settings: Settings,
    at: Option<f64>,
) -> Result<preview::Image, String> {
    let e = state.engine()?;
    blocking(move || {
        let t = e.tools();
        let m = probe::probe(t, Path::new(&path)).map_err(|e| e.to_string())?;
        let p = plan::resolve(t, &settings, &m).map_err(|e| e.to_string())?;
        let at = at.unwrap_or(p.start + (p.duration * 0.1).min(5.0));
        preview::output_preview(t, &m, &p, at).map_err(|e| e.to_string())
    })
    .await
}

/// Where Android writes results: the shared Movies / Music folders.
fn android_output_dir(audio: bool) -> Option<PathBuf> {
    if !cfg!(target_os = "android") {
        return None;
    }
    let kind = if audio { "Music" } else { "Movies" };
    // Shared Movies/Music folders (Android 11+), else the app's own external folder.
    let candidates = [
        PathBuf::from("/storage/emulated/0").join(kind).join("Pocket Reel"),
        PathBuf::from("/storage/emulated/0/Android/data/app.pocketreel.converter/files").join(kind),
    ];
    candidates.into_iter().find(|dir| {
        std::fs::create_dir_all(dir).is_ok() && {
            // Use a media extension: Android rejects other file types in Movies/ and Music/.
            let probe = dir.join(if audio { ".pocketreel-write-test.mp3" } else { ".pocketreel-write-test.avi" });
            let ok = std::fs::write(&probe, b"ok").is_ok();
            let _ = std::fs::remove_file(&probe);
            ok
        }
    })
}

#[tauri::command]
async fn enqueue(app: AppHandle, state: State<'_, AppState>, mut request: JobRequest) -> Result<JobSnapshot, String> {
    let e = state.engine()?;
    if cfg!(target_os = "android") && request.output_dir.is_none() {
        let audio = presets::find(&request.settings.preset).map(|p| p.video.is_none()).unwrap_or(false);
        let dir = android_output_dir(audio)
            .or_else(|| app.path().app_data_dir().ok().map(|d| d.join("Converted")))
            .ok_or("No writable folder for the output.")?;
        request.output_dir = Some(dir.to_string_lossy().to_string());
    }
    blocking(move || e.enqueue(request).map_err(|e| e.to_string())).await
}

#[tauri::command]
fn cancel(state: State<AppState>, id: u64) -> Result<bool, String> {
    Ok(state.engine()?.cancel(id))
}

#[tauri::command]
fn cancel_all(state: State<AppState>) -> Result<(), String> {
    state.engine()?.cancel_all();
    Ok(())
}

#[tauri::command]
fn remove(state: State<AppState>, id: u64) -> Result<bool, String> {
    Ok(state.engine()?.remove(id))
}

#[tauri::command]
async fn retry(state: State<'_, AppState>, id: u64) -> Result<JobSnapshot, String> {
    let e = state.engine()?;
    blocking(move || e.retry(id).map_err(|e| e.to_string())).await
}

#[tauri::command]
fn jobs(state: State<AppState>) -> Result<Vec<JobSnapshot>, String> {
    Ok(state.engine()?.jobs())
}

#[tauri::command]
fn set_concurrency(state: State<AppState>, n: usize) -> Result<(), String> {
    state.engine()?.set_concurrency(n);
    Ok(())
}

/// Title, length and thumbnail of a link (YouTube and other sites).
#[tauri::command]
async fn link_info(app: AppHandle, url: String) -> Result<RemoteInfo, String> {
    let e = ready_engine(&app).await?;
    let url = extract_url(&url).ok_or("That doesn't look like a link.")?;
    blocking(move || e.link_info(&url).map_err(|e| e.to_string())).await
}

/// A link shared to the app from another app (Android), taken once.
#[tauri::command]
async fn take_shared_url(app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        if let Some(h) = app.try_state::<mobile::YtdlHandle>() {
            let h = h.0.clone();
            return blocking(move || {
                let r: mobile::SharedResp = h.run_mobile_plugin("takeShared", ()).map_err(|e| e.to_string())?;
                Ok(r.url)
            })
            .await;
        }
    }
    let _ = app;
    Ok(None)
}

/// Android: copy picked content:// documents into app storage so FFmpeg can read them.
#[tauri::command]
async fn import_uris(app: AppHandle, uris: Vec<String>) -> Result<Vec<String>, String> {
    use tauri_plugin_fs::FsExt;
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("imports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for uri in uris {
        let fp: tauri_plugin_fs::FilePath = uri.parse().map_err(|_| format!("Unsupported location: {uri}"))?;
        let mut opts = tauri_plugin_fs::OpenOptions::new();
        opts.read(true);
        let mut src = app.fs().open(fp, opts).map_err(|e| e.to_string())?;
        // The descriptor behind a content:// URI usually points at the real file,
        // which gives the original name ("Holiday.mp4" instead of "video:1234").
        let mut tail = uri.rsplit(['/', ':']).next().unwrap_or("video").replace("%20", " ");
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if let Ok(real) = std::fs::read_link(format!("/proc/self/fd/{}", src.as_raw_fd())) {
                if let Some(n) = real.file_name() {
                    tail = n.to_string_lossy().to_string();
                }
            }
        }
        let clean: String =
            tail.chars().map(|c| if c.is_alphanumeric() || " ._-()".contains(c) { c } else { '_' }).collect();
        let name = if clean.contains('.') { clean } else { format!("Video {clean}") };
        let dest = dir.join(&name);
        let mut file = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut src, &mut file).map_err(|e| e.to_string())?;
        out.push(dest.to_string_lossy().to_string());
    }
    Ok(out)
}

/// Enable "paste a link" and sharing: yt-dlp on desktop, youtubedl-android on Android.
fn setup_downloads(app: &AppHandle, engine: &Engine, ffmpeg: &Path) {
    let work = app.path().app_cache_dir().unwrap_or_else(|_| std::env::temp_dir()).join("downloads");
    let _ = std::fs::remove_dir_all(&work); // leftovers from an interrupted run
    #[cfg(target_os = "android")]
    {
        let _ = ffmpeg;
        if let Some(h) = app.try_state::<mobile::YtdlHandle>() {
            let out = android_output_dir(false)
                .or_else(|| app.path().app_data_dir().ok().map(|d| d.join("Converted")))
                .unwrap_or_else(|| work.join("out"));
            let downloader = mobile::MobileYtdl { handle: h.0.clone() };
            downloader.update_daily(&app.path().app_data_dir().unwrap_or_else(|_| work.clone()));
            engine.set_downloader(Arc::new(downloader), work, out);
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let Some(mut ytdlp) = pocketreel_engine::YtDlp::locate(&tool_dirs(app), ffmpeg) else { return };
        if let Ok(data) = app.path().app_data_dir() {
            ytdlp.make_updatable(&data);
        }
        let out = app
            .path()
            .video_dir()
            .or_else(|_| app.path().home_dir())
            .map(|d| d.join("Pocket Reel"))
            .unwrap_or_else(|_| work.join("out"));
        engine.set_downloader(Arc::new(ytdlp), work, out);
    }
}

fn ytdl_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("ytdl")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin("app.pocketreel.converter", "YtdlPlugin")?;
                app.manage(mobile::YtdlHandle(handle));
            }
            let _ = (app, api);
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
mod mobile {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime};

    use pocketreel_engine::download::{friendly_error, Downloader, RemoteInfo};
    use pocketreel_engine::{CancelToken, Error, Result};
    use serde::Deserialize;
    use serde_json::json;
    use tauri::plugin::PluginHandle;

    pub struct YtdlHandle(pub PluginHandle<tauri::Wry>);

    #[derive(Deserialize)]
    pub struct SharedResp {
        pub url: Option<String>,
    }

    #[derive(Deserialize)]
    struct InfoResp {
        id: Option<String>,
        title: Option<String>,
        duration: Option<f64>,
        thumbnail: Option<String>,
        uploader: Option<String>,
        url: Option<String>,
    }

    #[derive(Deserialize)]
    struct PathResp {
        path: Option<String>,
    }

    #[derive(Deserialize)]
    struct ProgResp {
        progress: f64,
        eta: i64,
    }

    pub struct MobileYtdl {
        pub handle: PluginHandle<tauri::Wry>,
    }

    fn err(e: impl std::fmt::Display) -> Error {
        let text = e.to_string();
        if text.contains("Canceled") {
            Error::Canceled
        } else {
            Error::Download(friendly_error(&text))
        }
    }

    impl MobileYtdl {
        /// YouTube changes often; refresh yt-dlp at most once a day.
        pub fn update_daily(&self, data_dir: &Path) {
            let stamp = data_dir.join("yt-dlp-last-update");
            let stale = std::fs::metadata(&stamp)
                .and_then(|m| m.modified())
                .map(|t| SystemTime::now().duration_since(t).unwrap_or_default() > Duration::from_secs(86_400))
                .unwrap_or(true);
            if stale {
                let h = self.handle.clone();
                std::thread::spawn(move || {
                    if h.run_mobile_plugin::<serde_json::Value>("update", ()).is_ok() {
                        let _ = std::fs::write(&stamp, b"ok");
                    }
                });
            }
        }
    }

    impl Downloader for MobileYtdl {
        fn info(&self, url: &str) -> Result<RemoteInfo> {
            let r: InfoResp = self.handle.run_mobile_plugin("info", json!({ "url": url })).map_err(err)?;
            Ok(RemoteInfo {
                url: r.url.unwrap_or_else(|| url.to_string()),
                id: r.id.unwrap_or_default(),
                title: r.title.filter(|t| !t.is_empty()).unwrap_or_else(|| "Video".into()),
                duration: r.duration.filter(|d| *d > 0.0),
                thumbnail: r.thumbnail,
                uploader: r.uploader,
            })
        }

        fn download(
            &self,
            url: &str,
            dir: &Path,
            cancel: &CancelToken,
            on_progress: &mut dyn FnMut(f64, Option<f64>),
        ) -> Result<PathBuf> {
            std::fs::create_dir_all(dir)?;
            let id = format!(
                "dl-{}",
                SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
            );
            let (tx, rx) = mpsc::channel();
            let h = self.handle.clone();
            let args = json!({ "url": url, "dir": dir.to_string_lossy(), "id": id });
            std::thread::spawn(move || {
                let _ = tx.send(h.run_mobile_plugin::<PathResp>("download", args));
            });
            // youtubedl-android reports percent per stream; video first, then audio.
            let mut stream = 0usize;
            let mut last = 0.0f64;
            loop {
                if cancel.is_canceled() {
                    let _ = self.handle.run_mobile_plugin::<serde_json::Value>("cancel", json!({ "id": id }));
                }
                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(res) => {
                        if cancel.is_canceled() {
                            return Err(Error::Canceled);
                        }
                        let r = res.map_err(err)?;
                        let path = r
                            .path
                            .map(PathBuf::from)
                            .filter(|p| p.is_file())
                            .ok_or_else(|| Error::Download("The download finished but no video file was found.".into()))?;
                        on_progress(1.0, Some(0.0));
                        return Ok(path);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Ok(p) = self.handle.run_mobile_plugin::<ProgResp>("progress", json!({ "id": id })) {
                            let pct = p.progress.clamp(0.0, 100.0);
                            if pct + 5.0 < last {
                                stream += 1;
                            }
                            last = pct;
                            let f = pct / 100.0;
                            let overall = if stream == 0 { f * 0.82 } else { 0.82 + f * 0.16 };
                            on_progress(overall, (p.eta >= 0).then_some(p.eta as f64));
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(Error::Download("The download stopped unexpectedly.".into()))
                    }
                }
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(ytdl_plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState { engine: OnceLock::new() })
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let result = match Tools::locate(&tool_dirs(&handle)) {
                    Ok(tools) => {
                        let emitter = handle.clone();
                        let ffmpeg = tools.ffmpeg.clone();
                        let engine = Engine::new(tools, move |ev: Event| {
                            let _ = emitter.emit("engine-event", ev);
                        });
                        setup_downloads(&handle, &engine, &ffmpeg);
                        Ok(engine)
                    }
                    Err(e) => Err(e.to_string()),
                };
                let _ = handle.state::<AppState>().engine.set(result);
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(s) = window.app_handle().try_state::<AppState>() {
                    if let Some(Ok(e)) = s.engine.get() {
                        e.shutdown();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            presets,
            probe,
            thumbnail,
            plan,
            estimate,
            preview,
            enqueue,
            cancel,
            cancel_all,
            remove,
            retry,
            jobs,
            set_concurrency,
            import_uris,
            link_info,
            take_shared_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pocket Reel");
}
