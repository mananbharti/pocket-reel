//! Tauri shell: exposes the engine to the web UI on desktop and Android.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pocketreel_engine::{
    plan, presets, preview, probe, Engine, Estimate, Event, JobRequest, JobSnapshot, MediaInfo, Plan, Preset, Settings,
    Tools,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    engine: Mutex<Option<Engine>>,
    error: Option<String>,
    platform: &'static str,
}

impl AppState {
    fn engine(&self) -> Result<Engine, String> {
        self.engine
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| self.error.clone().unwrap_or_else(|| "FFmpeg is not available.".into()))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    platform: &'static str,
    ffmpeg: String,
    default_output_dir: Option<String>,
    version: String,
    engine_error: Option<String>,
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
fn app_info(app: AppHandle, state: State<AppState>) -> AppInfo {
    let engine = state.engine.lock().unwrap().clone();
    AppInfo {
        platform: state.platform,
        ffmpeg: engine.as_ref().map(|e| e.tools().version.clone()).unwrap_or_default(),
        default_output_dir: android_output_dir(false).map(|p| p.to_string_lossy().to_string()),
        version: app.package_info().version.to_string(),
        engine_error: state.error.clone(),
    }
}

#[tauri::command]
fn presets(state: State<AppState>) -> Result<Vec<Preset>, String> {
    Ok(presets::catalog_for(state.engine()?.tools()))
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
            let probe = dir.join(".pocketreel-write-test");
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let (engine, error) = match Tools::locate(&tool_dirs(&handle)) {
                Ok(tools) => {
                    let emitter = handle.clone();
                    let engine = Engine::new(tools, move |ev: Event| {
                        let _ = emitter.emit("engine-event", ev);
                    });
                    (Some(engine), None)
                }
                Err(e) => (None, Some(e.to_string())),
            };
            app.manage(AppState { engine: Mutex::new(engine), error, platform: platform() });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(s) = window.app_handle().try_state::<AppState>() {
                    if let Some(e) = s.engine.lock().unwrap().as_ref() {
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
            import_uris
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pocket Reel");
}
