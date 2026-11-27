//! In-app updates: find a newer release on GitHub, download it, check it
//! against the release's SHA256SUMS.txt and install it the way this copy of
//! the app was installed.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
#[cfg(not(target_os = "android"))]
use std::time::Duration;

use pocketreel_engine::update::{self, Package, UpdateInfo};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Only files from this repository's releases are ever installed.
const DOWNLOAD_PREFIX: &str = "https://github.com/mananbharti/pocket-reel/releases/download/";

#[derive(Default)]
pub struct UpdateState {
    found: Mutex<Option<UpdateInfo>>,
    busy: Mutex<bool>,
}

/// The program that is running, noted at startup (before an update can
/// replace it) so the new version can be started in its place.
static EXE: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn remember_exe() {
    EXE.get_or_init(|| std::env::current_exe().ok());
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
    /// "download", "verify" or "install".
    stage: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    /// "restarting": the app closes and the new version opens.
    /// "installer": the system installer is showing (Android).
    /// "manual": a window opened to finish by hand (macOS fallback).
    outcome: &'static str,
    /// Android: the phone will first ask to allow installs from Pocket Reel.
    needs_permission: bool,
}

fn progress(app: &AppHandle, downloaded: u64, total: Option<u64>, stage: &'static str) {
    let _ = app.emit("update-progress", UpdateProgress { downloaded, total, stage });
}

// ------------------------------------------------------------ transport

#[cfg(not(target_os = "android"))]
fn fetch_text(_app: &AppHandle, url: &str) -> Result<String, String> {
    update::http::get_text(url).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "android"))]
fn fetch_file(app: &AppHandle, url: &str, dest: &Path) -> Result<(), String> {
    let mut last = std::time::Instant::now() - Duration::from_secs(1);
    update::http::download(url, dest, &pocketreel_engine::CancelToken::new(), &mut |done, total| {
        if last.elapsed() > Duration::from_millis(150) {
            last = std::time::Instant::now();
            progress(app, done, total, "download");
        }
    })
    .map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
fn plugin(app: &AppHandle) -> Result<tauri::plugin::PluginHandle<tauri::Wry>, String> {
    app.try_state::<crate::mobile::YtdlHandle>().map(|h| h.0.clone()).ok_or_else(|| "Updates aren't available.".to_string())
}

#[cfg(target_os = "android")]
fn fetch_text(app: &AppHandle, url: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct R {
        text: String,
    }
    let r: R = plugin(app)?.run_mobile_plugin("fetchText", serde_json::json!({ "url": url })).map_err(|e| e.to_string())?;
    Ok(r.text)
}

#[cfg(target_os = "android")]
fn fetch_file(app: &AppHandle, url: &str, dest: &Path) -> Result<(), String> {
    use std::sync::mpsc;
    use std::time::Duration;
    #[derive(serde::Deserialize)]
    struct P {
        done: u64,
        total: i64,
    }
    let h = plugin(app)?;
    let (tx, rx) = mpsc::channel();
    let args = serde_json::json!({ "url": url, "path": dest.to_string_lossy(), "id": "update" });
    let h2 = h.clone();
    std::thread::spawn(move || {
        let _ = tx.send(h2.run_mobile_plugin::<serde_json::Value>("fetchFile", args));
    });
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(r) => return r.map(|_| ()).map_err(|e| e.to_string()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(p) = h.run_mobile_plugin::<P>("fetchProgress", serde_json::json!({ "id": "update" })) {
                    progress(app, p.done, (p.total > 0).then_some(p.total as u64), "download");
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("The update download stopped.".into()),
        }
    }
}

// ------------------------------------------------------------ commands

/// Looks for a newer release. `None` when up to date (or not supported here).
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(package) = Package::current() else { return Ok(None) };
        let json = fetch_text(&app, update::LATEST_RELEASE_URL)?;
        let found = update::from_release(&json, env!("CARGO_PKG_VERSION"), package).map_err(|e| e.to_string())?;
        let found = found.filter(|u| u.url.starts_with(DOWNLOAD_PREFIX));
        *app.state::<UpdateState>().found.lock().unwrap() = found.clone();
        Ok(found)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Downloads, verifies and installs the update found by `check_update`.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<InstallOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<UpdateState>();
        let info = state.found.lock().unwrap().clone().ok_or("Check for updates first.")?;
        {
            let mut busy = state.busy.lock().unwrap();
            if *busy {
                return Err("The update is already being installed.".into());
            }
            *busy = true;
        }
        let result = download_and_install(&app, &info);
        *state.busy.lock().unwrap() = false;
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Opens the release page ("What's new").
#[tauri::command]
pub fn open_release_page(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let page = app.state::<UpdateState>().found.lock().unwrap().as_ref().map(|u| u.page.clone());
    let page = page.unwrap_or_else(|| "https://github.com/mananbharti/pocket-reel/releases/latest".into());
    if !page.starts_with("https://github.com/mananbharti/pocket-reel/") {
        return Err("Unexpected release page.".into());
    }
    app.opener().open_url(page, None::<&str>).map_err(|e| e.to_string())
}

fn download_and_install(app: &AppHandle, info: &UpdateInfo) -> Result<InstallOutcome, String> {
    // Must match update_paths.xml on Android (<cache>/updates/).
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("updates");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(info.package.asset_name());

    progress(app, 0, Some(info.size), "download");
    fetch_file(app, &info.url, &dest)?;
    progress(app, info.size, Some(info.size), "verify");
    let sums_url = info.sums_url.as_deref().ok_or("The release has no checksum list, so it can't be checked.")?;
    if !sums_url.starts_with(DOWNLOAD_PREFIX) {
        return Err("Unexpected checksum list.".into());
    }
    let sums = fetch_text(app, sums_url)?;
    update::verify(&dest, info.package.asset_name(), &sums).map_err(|e| e.to_string())?;
    progress(app, info.size, Some(info.size), "install");
    install(app, info.package, &dest)
}

// ------------------------------------------------------------ installing

#[cfg(target_os = "android")]
fn install(app: &AppHandle, _package: Package, file: &Path) -> Result<InstallOutcome, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct R {
        #[serde(default)]
        needs_permission: bool,
    }
    let r: R = plugin(app)?
        .run_mobile_plugin("installApk", serde_json::json!({ "path": file.to_string_lossy() }))
        .map_err(|e| e.to_string())?;
    Ok(InstallOutcome { outcome: "installer", needs_permission: r.needs_permission })
}

#[cfg(not(target_os = "android"))]
fn restart_soon(app: &AppHandle) -> InstallOutcome {
    let h = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        h.exit(0);
    });
    InstallOutcome { outcome: "restarting", needs_permission: false }
}

#[cfg(not(target_os = "android"))]
fn exe() -> Result<PathBuf, String> {
    EXE.get_or_init(|| std::env::current_exe().ok()).clone().ok_or_else(|| "Couldn't find the running app.".to_string())
}

#[cfg(target_os = "windows")]
fn install(app: &AppHandle, package: Package, file: &Path) -> Result<InstallOutcome, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = exe()?;
    // Wait for the installer (shown with a progress bar), then start the new version.
    let run = match package {
        Package::WindowsMsi => format!("msiexec /i \"{}\" /passive", file.display()),
        _ => format!("\"{}\" /P /UPDATE", file.display()),
    };
    let script = format!("start \"\" /wait {run} & start \"\" \"{}\"", exe.display());
    std::process::Command::new("cmd")
        .raw_arg(format!("/S /C \"{script}\""))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Couldn't start the installer: {e}"))?;
    Ok(restart_soon(app))
}

#[cfg(target_os = "macos")]
fn install(app: &AppHandle, _package: Package, file: &Path) -> Result<InstallOutcome, String> {
    use std::process::Command;
    // .../Pocket Reel.app/Contents/MacOS/pocket-reel -> .../Pocket Reel.app
    let bundle = exe()?.ancestors().nth(3).map(Path::to_path_buf).filter(|b| b.extension().is_some_and(|e| e == "app"));
    let replaced = bundle.as_deref().map(|b| replace_mac_app(file, b));
    match (bundle, replaced) {
        (Some(b), Some(Ok(()))) => {
            Command::new("/usr/bin/open").arg("-n").arg(&b).spawn().map_err(|e| e.to_string())?;
            Ok(restart_soon(app))
        }
        (_, err) => {
            if let Some(Err(e)) = err {
                eprintln!("update: replacing the app failed: {e}");
            }
            // Show the disk image; dragging the app to Applications finishes it.
            Command::new("/usr/bin/open").arg(file).spawn().map_err(|e| e.to_string())?;
            Ok(InstallOutcome { outcome: "manual", needs_permission: false })
        }
    }
}

#[cfg(target_os = "macos")]
fn replace_mac_app(dmg: &Path, bundle: &Path) -> Result<(), String> {
    use std::process::Command;
    let mnt = dmg.with_extension("mnt");
    let ok = |c: &mut Command| -> Result<(), String> {
        let s = c.status().map_err(|e| e.to_string())?;
        s.success().then_some(()).ok_or_else(|| format!("{c:?} failed"))
    };
    let _ = std::fs::create_dir_all(&mnt);
    ok(Command::new("/usr/bin/hdiutil").args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-quiet", "-mountpoint"]).arg(&mnt).arg(dmg))?;
    let result = (|| {
        let src = std::fs::read_dir(&mnt)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .ok_or("The disk image has no app")?;
        let staged = bundle.with_extension("app-new");
        let old = bundle.with_extension("app-old");
        let _ = std::fs::remove_dir_all(&staged);
        let _ = std::fs::remove_dir_all(&old);
        ok(Command::new("/usr/bin/ditto").arg(&src).arg(&staged))?;
        std::fs::rename(bundle, &old).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::rename(&staged, bundle) {
            let _ = std::fs::rename(&old, bundle);
            return Err(e.to_string());
        }
        let _ = std::fs::remove_dir_all(&old);
        let _ = Command::new("/usr/bin/xattr").args(["-dr", "com.apple.quarantine"]).arg(bundle).status();
        Ok(())
    })();
    let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-quiet"]).arg(&mnt).status();
    result
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
fn install(app: &AppHandle, package: Package, file: &Path) -> Result<InstallOutcome, String> {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    // Leave the old AppImage's environment behind.
    let clean = |c: &mut Command| {
        for v in ["APPIMAGE", "APPDIR", "ARGV0", "OWD", "LD_LIBRARY_PATH", "GIO_MODULE_DIR", "GDK_PIXBUF_MODULE_FILE"] {
            c.env_remove(v);
        }
    };
    match package {
        Package::AppImage => {
            let target = PathBuf::from(std::env::var_os("APPIMAGE").ok_or("Couldn't find the AppImage file.")?);
            let staged = target.with_extension("AppImage.new");
            std::fs::copy(file, &staged).map_err(|e| format!("Couldn't write next to {}: {e}", target.display()))?;
            std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
            std::fs::rename(&staged, &target).map_err(|e| e.to_string())?;
            let mut c = Command::new(&target);
            clean(&mut c);
            c.spawn().map_err(|e| e.to_string())?;
            Ok(restart_soon(app))
        }
        _ => {
            // .deb: the system asks for the password, then apt installs it.
            let status = Command::new("pkexec")
                .args(["apt-get", "install", "-y", "--allow-downgrades"])
                .arg(file)
                .status()
                .map_err(|_| "Couldn't ask for permission to install (pkexec is missing).".to_string())?;
            if !status.success() {
                return Err(if matches!(status.code(), Some(126) | Some(127)) {
                    "The update was cancelled.".into()
                } else {
                    "The package manager couldn't install the update.".into()
                });
            }
            let exe = exe()?;
            let exe = PathBuf::from(exe.to_string_lossy().trim_end_matches(" (deleted)"));
            let mut c = Command::new(exe);
            clean(&mut c);
            c.spawn().map_err(|e| e.to_string())?;
            Ok(restart_soon(app))
        }
    }
}
