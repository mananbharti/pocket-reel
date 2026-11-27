//! Finding a newer Pocket Reel on GitHub Releases.
//!
//! The app checks the latest release, picks the download that matches how it
//! was installed (APK, Windows installer, macOS disk image, AppImage or .deb),
//! and checks the file against the release's `SHA256SUMS.txt` before
//! installing it.

use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// GitHub's description of the newest (non-pre-release) release.
pub const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/mananbharti/pocket-reel/releases/latest";
pub const USER_AGENT: &str = "PocketReel-updater";

/// The kind of package this copy of the app was installed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Package {
    Android,
    WindowsSetup,
    WindowsMsi,
    MacDmg,
    AppImage,
    Deb,
}

impl Package {
    pub fn asset_name(self) -> &'static str {
        match self {
            Package::Android => "PocketReel-android.apk",
            Package::WindowsSetup => "PocketReel-windows-x64-setup.exe",
            Package::WindowsMsi => "PocketReel-windows-x64.msi",
            Package::MacDmg => "PocketReel-macos-arm64.dmg",
            Package::AppImage => "PocketReel-linux-x86_64.AppImage",
            Package::Deb => "PocketReel-linux-amd64.deb",
        }
    }

    /// The package for the running app, if it can update itself.
    pub fn current() -> Option<Package> {
        if cfg!(target_os = "android") {
            Some(Package::Android)
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            // The MSI installs for all users under Program Files; the setup
            // program installs just for this user.
            let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
            Some(if exe.contains("\\program files") { Package::WindowsMsi } else { Package::WindowsSetup })
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Package::MacDmg)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            // The AppImage runtime tells the app where its file is.
            if std::env::var_os("APPIMAGE").is_some() {
                Some(Package::AppImage)
            } else {
                Some(Package::Deb)
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// For example "1.3.0".
    pub version: String,
    pub package: Package,
    pub asset: String,
    pub url: String,
    pub size: u64,
    /// The release's SHA256SUMS.txt, when it has one.
    pub sums_url: Option<String>,
    /// The release page ("What's new").
    pub page: String,
}

/// "v1.2.3" / "1.2.3" -> (1, 2, 3). Anything after a "-" or "+" is ignored.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Reads GitHub's release JSON; `None` when it isn't newer than `current` or
/// has no download for `package` (for example while it is still being built).
pub fn from_release(json: &str, current: &str, package: Package) -> Result<Option<UpdateInfo>> {
    let r: Release = serde_json::from_str(json).map_err(|e| Error::Invalid(format!("Unexpected release data: {e}")))?;
    if r.draft || r.prerelease || !is_newer(&r.tag_name, current) {
        return Ok(None);
    }
    let Some(asset) = r.assets.iter().find(|a| a.name == package.asset_name()) else {
        return Ok(None);
    };
    let sums_url = r.assets.iter().find(|a| a.name == "SHA256SUMS.txt").map(|a| a.browser_download_url.clone());
    let version = r.tag_name.trim().trim_start_matches(['v', 'V']).to_string();
    Ok(Some(UpdateInfo {
        version,
        package,
        asset: asset.name.clone(),
        url: asset.browser_download_url.clone(),
        size: asset.size,
        sums_url,
        page: r.html_url,
    }))
}

/// Finds `asset`'s hash in a `sha256sum` listing ("<hash>  <name>" or "<hash> *<name>").
pub fn checksum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        let name = name.rsplit('/').next().unwrap_or(name);
        (name == asset && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Fails unless `file` matches the hash listed for `asset` in `sums`.
pub fn verify(file: &Path, asset: &str, sums: &str) -> Result<()> {
    let want = checksum_for(sums, asset)
        .ok_or_else(|| Error::Download(format!("The update's checksum list doesn't include {asset}.")))?;
    let got = sha256_file(file)?;
    if got != want {
        return Err(Error::Download("The update didn't download correctly (checksum mismatch). Try again.".into()));
    }
    Ok(())
}

/// Plain HTTPS for the desktop apps (Android downloads through the system).
#[cfg(feature = "http")]
pub mod http {
    use std::io::{Read, Write};
    use std::path::Path;
    use std::time::Duration;

    use super::USER_AGENT;
    use crate::{CancelToken, Error, Result};

    fn agent() -> ureq::Agent {
        ureq::AgentBuilder::new()
            .user_agent(USER_AGENT)
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(60))
            .build()
    }

    fn net_err(e: ureq::Error) -> Error {
        match e {
            ureq::Error::Status(code, _) => Error::Download(format!("The update server answered with error {code}.")),
            ureq::Error::Transport(t) => Error::Download(format!("Couldn't reach the update server ({t}).")),
        }
    }

    pub fn get_text(url: &str) -> Result<String> {
        let resp = agent().get(url).set("Accept", "application/vnd.github+json").call().map_err(net_err)?;
        let mut s = String::new();
        resp.into_reader().take(8 << 20).read_to_string(&mut s)?;
        Ok(s)
    }

    /// Downloads to `dest` (via a temporary file), reporting (done, total).
    pub fn download(url: &str, dest: &Path, cancel: &CancelToken, on_progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<()> {
        let resp = agent().get(url).call().map_err(net_err)?;
        let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
        let tmp = dest.with_extension("part");
        let mut out = std::fs::File::create(&tmp)?;
        let mut reader = resp.into_reader();
        let mut buf = vec![0u8; 1 << 16];
        let mut done = 0u64;
        loop {
            if cancel.is_canceled() {
                drop(out);
                let _ = std::fs::remove_file(&tmp);
                return Err(Error::Canceled);
            }
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            done += n as u64;
            on_progress(done, total);
        }
        out.sync_all()?;
        drop(out);
        if let Some(t) = total {
            if done != t {
                let _ = std::fs::remove_file(&tmp);
                return Err(Error::Download("The update download was cut short. Try again.".into()));
            }
        }
        std::fs::rename(&tmp, dest)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE: &str = r#"{
        "tag_name": "v1.3.0", "html_url": "https://github.com/mananbharti/pocket-reel/releases/tag/v1.3.0",
        "draft": false, "prerelease": false,
        "assets": [
            {"name": "PocketReel-android.apk", "browser_download_url": "https://example.invalid/a.apk", "size": 100},
            {"name": "PocketReel-linux-x86_64.AppImage", "browser_download_url": "https://example.invalid/x.AppImage", "size": 200},
            {"name": "SHA256SUMS.txt", "browser_download_url": "https://example.invalid/SHA256SUMS.txt", "size": 5}
        ]
    }"#;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.10"), Some((1, 10, 0)));
        assert_eq!(parse_version("v2.0.0-beta.1"), Some((2, 0, 0)));
        assert_eq!(parse_version("latest"), None);
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(is_newer("v1.2.10", "1.2.9"));
        assert!(!is_newer("v1.2.2", "1.2.2"));
        assert!(!is_newer("v1.2.1", "1.2.2"));
        assert!(!is_newer("nonsense", "1.2.2"));
    }

    #[test]
    fn picks_the_matching_download() {
        let u = from_release(RELEASE, "1.2.2", Package::Android).unwrap().unwrap();
        assert_eq!(u.version, "1.3.0");
        assert_eq!(u.url, "https://example.invalid/a.apk");
        assert_eq!(u.size, 100);
        assert_eq!(u.sums_url.as_deref(), Some("https://example.invalid/SHA256SUMS.txt"));
        assert!(u.page.ends_with("/v1.3.0"));
        // Same version, or no file for this platform yet: nothing to do.
        assert_eq!(from_release(RELEASE, "1.3.0", Package::Android).unwrap(), None);
        assert_eq!(from_release(RELEASE, "1.2.2", Package::MacDmg).unwrap(), None);
        assert!(from_release("{}", "1.0.0", Package::Android).is_err());
    }

    #[test]
    fn checksums() {
        let h = "a".repeat(64);
        let sums = format!("{h}  PocketReel-android.apk\n{}  *dist/PocketReel-windows-x64-setup.exe\n", "B".repeat(64));
        assert_eq!(checksum_for(&sums, "PocketReel-android.apk"), Some(h));
        assert_eq!(checksum_for(&sums, "PocketReel-windows-x64-setup.exe"), Some("b".repeat(64)));
        assert_eq!(checksum_for(&sums, "missing.dmg"), None);

        let dir = std::env::temp_dir().join(format!("pr-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("PocketReel-android.apk");
        std::fs::write(&f, b"hello").unwrap();
        let good = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert_eq!(sha256_file(&f).unwrap(), good);
        assert!(verify(&f, "PocketReel-android.apk", &format!("{good}  PocketReel-android.apk")).is_ok());
        assert!(verify(&f, "PocketReel-android.apk", &sums).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
