//! A small HTTP bridge exposing the engine to the web UI during development
//! and for browser-based testing. Binds to 127.0.0.1 only.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use pocketreel_engine::{plan, presets, preview, probe, Engine, JobRequest, Settings, Tools, YtDlp};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn b64decode(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(s).unwrap_or_default()
}

type Subscribers = Arc<Mutex<Vec<Sender<String>>>>;

struct Ctx {
    engine: Engine,
    subs: Subscribers,
    ui: Option<PathBuf>,
    uploads: PathBuf,
}

/// A parsed HTTP/1.1 request. Bodies are read completely up front.
struct Request {
    method: String,
    url: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut line = String::new();
    stream.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let url = parts.next()?.to_string();
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        if stream.read_line(&mut h).ok()? == 0 {
            break;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).ok()?;
    Some(Request { method, url, body })
}

fn write_head(out: &mut TcpStream, status: u16, ctype: &str, len: Option<u64>) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    };
    write!(out, "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nCache-Control: no-store\r\nConnection: close\r\n")?;
    if let Some(l) = len {
        write!(out, "Content-Length: {l}\r\n")?;
    }
    write!(out, "\r\n")
}

fn send(out: &mut TcpStream, status: u16, ctype: &str, data: &[u8]) {
    let _ = write_head(out, status, ctype, Some(data.len() as u64)).and_then(|_| out.write_all(data));
    let _ = out.flush();
}

fn send_json(out: &mut TcpStream, status: u16, v: &Value) {
    send(out, status, "application/json", &serde_json::to_vec(v).unwrap());
}

fn ok<T: serde::Serialize>(v: T) -> (u16, Value) {
    (200, serde_json::to_value(v).unwrap_or(Value::Null))
}

fn err(msg: impl ToString) -> (u16, Value) {
    (400, json!({ "error": msg.to_string() }))
}

fn body<T: for<'de> Deserialize<'de>>(req: &mut Request) -> Result<T, String> {
    serde_json::from_slice(&req.body).map_err(|e| format!("bad request: {e}"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathArg {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThumbArg {
    path: String,
    #[serde(default)]
    at: Option<f64>,
    #[serde(default)]
    max_width: Option<u32>,
    #[serde(default)]
    max_height: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanArg {
    path: String,
    settings: Settings,
    #[serde(default)]
    at: Option<f64>,
}

#[derive(Deserialize)]
struct UrlArg {
    url: String,
}

#[derive(Deserialize)]
struct IdArg {
    id: u64,
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    for pair in q.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == key {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "woff2" => "font/woff2",
        "json" => "application/json",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "avi" => "video/x-msvideo",
        _ => "application/octet-stream",
    }
}

fn api(ctx: &Ctx, req: &mut Request) -> (u16, Value) {
    let url = req.url.clone();
    let path = url.split('?').next().unwrap_or("").to_string();
    let t = ctx.engine.tools();
    match (req.method.as_str(), path.as_str()) {
        ("GET", "/api/info") => ok(json!({
            "platform": "web",
            "ffmpeg": t.version,
            "defaultOutputDir": ctx.uploads.join("converted").to_string_lossy(),
            "downloads": ctx.engine.can_download(),
        })),
        ("GET", "/api/presets") => ok(presets::catalog_for(t)),
        ("GET", "/api/jobs") => ok(ctx.engine.jobs()),
        ("POST", "/api/probe") => match body::<PathArg>(req) {
            Ok(a) => match probe::probe(t, Path::new(&a.path)) {
                Ok(m) => ok(m),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        ("POST", "/api/thumbnail") => match body::<ThumbArg>(req) {
            Ok(a) => {
                let r = probe::probe(t, Path::new(&a.path)).and_then(|m| {
                    let at = a.at.unwrap_or((m.duration * 0.1).min(5.0));
                    preview::thumbnail(t, &m, at, a.max_width.unwrap_or(480), a.max_height.unwrap_or(270))
                });
                match r {
                    Ok(i) => ok(i),
                    Err(e) => err(e),
                }
            }
            Err(e) => err(e),
        },
        ("POST", "/api/plan") => match body::<PlanArg>(req) {
            Ok(a) => match probe::probe(t, Path::new(&a.path)).and_then(|m| plan::resolve(t, &a.settings, &m)) {
                Ok(p) => ok(p),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        ("POST", "/api/estimate") => match body::<PlanArg>(req) {
            Ok(a) => {
                let r = probe::probe(t, Path::new(&a.path))
                    .and_then(|m| plan::resolve(t, &a.settings, &m))
                    .and_then(|p| preview::estimate(t, &p, Path::new(&a.path)));
                match r {
                    Ok(e) => ok(e),
                    Err(e) => err(e),
                }
            }
            Err(e) => err(e),
        },
        ("POST", "/api/preview") => match body::<PlanArg>(req) {
            Ok(a) => {
                let r = probe::probe(t, Path::new(&a.path)).and_then(|m| {
                    let p = plan::resolve(t, &a.settings, &m)?;
                    let at = a.at.unwrap_or(p.start + (p.duration * 0.1).min(5.0));
                    preview::output_preview(t, &m, &p, at)
                });
                match r {
                    Ok(i) => ok(i),
                    Err(e) => err(e),
                }
            }
            Err(e) => err(e),
        },
        ("POST", "/api/link-info") => match body::<UrlArg>(req) {
            Ok(a) => match ctx.engine.link_info(a.url.trim()) {
                Ok(i) => ok(i),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        ("POST", "/api/enqueue") => match body::<JobRequest>(req) {
            Ok(r) => match ctx.engine.enqueue(r) {
                Ok(s) => ok(s),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        ("POST", "/api/cancel") => match body::<IdArg>(req) {
            Ok(a) => ok(ctx.engine.cancel(a.id)),
            Err(e) => err(e),
        },
        ("POST", "/api/cancel-all") => {
            ctx.engine.cancel_all();
            ok(true)
        }
        ("POST", "/api/remove") => match body::<IdArg>(req) {
            Ok(a) => ok(ctx.engine.remove(a.id)),
            Err(e) => err(e),
        },
        ("POST", "/api/retry") => match body::<IdArg>(req) {
            Ok(a) => match ctx.engine.retry(a.id) {
                Ok(s) => ok(s),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        ("POST", "/api/upload") => {
            let name = query_param(&url, "name").unwrap_or_else(|| "upload.bin".into());
            let safe: String = name
                .chars()
                .map(|c| if c.is_alphanumeric() || " ._-()".contains(c) { c } else { '_' })
                .collect();
            let dest = ctx.uploads.join(safe.trim_start_matches('.'));
            match std::fs::write(&dest, &req.body) {
                Ok(()) => ok(json!({ "path": dest.to_string_lossy() })),
                Err(e) => err(e),
            }
        }
        _ => (404, json!({ "error": "not found" })),
    }
}

fn handle(ctx: Arc<Ctx>, stream: TcpStream) {
    let mut out = match stream.try_clone() {
        Ok(o) => o,
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);
    let Some(mut req) = read_request(&mut reader) else { return };
    let url = req.url.clone();
    let path = url.split('?').next().unwrap_or("/").to_string();
    if std::env::var_os("POCKETREEL_LOG").is_some() {
        eprintln!("{} {}", req.method, url.chars().take(80).collect::<String>());
    }
    if path == "/api/events" {
        let (tx, rx) = mpsc::channel();
        ctx.subs.lock().unwrap().push(tx);
        if write_head(&mut out, 200, "text/event-stream", None).is_err() {
            return;
        }
        let initial = serde_json::to_string(&json!({ "type": "snapshot", "jobs": ctx.engine.jobs() })).unwrap();
        let mut msg = format!("data: {initial}\n\n");
        loop {
            if out.write_all(msg.as_bytes()).and_then(|_| out.flush()).is_err() {
                return;
            }
            msg = match rx.recv_timeout(Duration::from_secs(15)) {
                Ok(m) => format!("data: {m}\n\n"),
                Err(RecvTimeoutError::Timeout) => ": ping\n\n".to_string(),
                Err(RecvTimeoutError::Disconnected) => return,
            };
        }
    }
    if path == "/api/file" {
        let Some(p) = query_param(&url, "path") else {
            return send_json(&mut out, 400, &json!({"error": "path required"}));
        };
        let p = PathBuf::from(p);
        match std::fs::File::open(&p) {
            Ok(mut f) => {
                let len = f.metadata().map(|m| m.len()).ok();
                if write_head(&mut out, 200, mime(&p), len).is_ok() {
                    let _ = std::io::copy(&mut f, &mut out);
                }
            }
            Err(e) => send_json(&mut out, 404, &json!({"error": e.to_string()})),
        }
        return;
    }
    if path.starts_with("/api/") {
        let (status, v) = api(&ctx, &mut req);
        return send_json(&mut out, status, &v);
    }
    if let Some(root) = &ctx.ui {
        let rel = path.trim_start_matches('/');
        let mut file = root.join(if rel.is_empty() { "index.html" } else { rel });
        if rel.contains("..") || !file.is_file() {
            file = root.join("index.html");
        }
        if let Ok(data) = std::fs::read(&file) {
            return send(&mut out, 200, mime(&file), &data);
        }
    }
    send(&mut out, 404, "text/plain", b"Not found");
}

pub fn serve(tools: Tools, port: u16, ui: Option<PathBuf>, uploads: Option<PathBuf>) -> Result<(), String> {
    let uploads = uploads.unwrap_or_else(|| std::env::temp_dir().join("pocketreel-uploads"));
    std::fs::create_dir_all(uploads.join("converted")).map_err(|e| e.to_string())?;
    let subs: Subscribers = Arc::new(Mutex::new(Vec::new()));
    let sink_subs = subs.clone();
    let ytdlp = YtDlp::locate(&[], &tools.ffmpeg);
    let engine = Engine::new(tools, move |ev| {
        let msg = serde_json::to_string(&ev).unwrap();
        sink_subs.lock().unwrap().retain(|s| s.send(msg.clone()).is_ok());
    });
    if let Some(y) = ytdlp {
        engine.set_downloader(Arc::new(y), uploads.join("downloads"), uploads.join("converted"));
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    eprintln!("Pocket Reel bridge on http://127.0.0.1:{port}  (uploads: {})", uploads.display());
    let ctx = Arc::new(Ctx { engine, subs, ui, uploads });
    for stream in listener.incoming().flatten() {
        let ctx = ctx.clone();
        std::thread::spawn(move || handle(ctx, stream));
    }
    Ok(())
}
