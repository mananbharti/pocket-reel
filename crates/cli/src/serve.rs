//! A small HTTP bridge exposing the engine to the web UI during development
//! and for browser-based testing. Binds to 127.0.0.1 only.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use pocketreel_engine::{plan, presets, preview, probe, Engine, JobRequest, Settings, Tools};
use serde::Deserialize;
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

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

struct SseReader {
    rx: Receiver<String>,
    buf: Cursor<Vec<u8>>,
}

impl Read for SseReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.buf.read(out)?;
            if n > 0 {
                return Ok(n);
            }
            let msg = match self.rx.recv_timeout(Duration::from_secs(15)) {
                Ok(m) => format!("data: {m}\n\n"),
                Err(RecvTimeoutError::Timeout) => ": ping\n\n".to_string(),
                Err(RecvTimeoutError::Disconnected) => return Ok(0),
            };
            self.buf = Cursor::new(msg.into_bytes());
        }
    }
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap()
}

fn json_response(status: u16, v: &Value) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(serde_json::to_vec(v).unwrap())
        .with_status_code(StatusCode(status))
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Cache-Control", "no-store"))
}

fn ok<T: serde::Serialize>(v: T) -> (u16, Value) {
    (200, serde_json::to_value(v).unwrap_or(Value::Null))
}

fn err(msg: impl ToString) -> (u16, Value) {
    (400, json!({ "error": msg.to_string() }))
}

fn body<T: for<'de> Deserialize<'de>>(req: &mut Request) -> Result<T, String> {
    serde_json::from_reader(req.as_reader()).map_err(|e| format!("bad request: {e}"))
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
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("").to_string();
    let t = ctx.engine.tools();
    match (req.method().clone(), path.as_str()) {
        (Method::Get, "/api/info") => ok(json!({
            "platform": "web",
            "ffmpeg": t.version,
            "defaultOutputDir": ctx.uploads.join("converted").to_string_lossy(),
        })),
        (Method::Get, "/api/presets") => ok(presets::catalog_for(t)),
        (Method::Get, "/api/jobs") => ok(ctx.engine.jobs()),
        (Method::Post, "/api/probe") => match body::<PathArg>(req) {
            Ok(a) => match probe::probe(t, Path::new(&a.path)) {
                Ok(m) => ok(m),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        (Method::Post, "/api/thumbnail") => match body::<ThumbArg>(req) {
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
        (Method::Post, "/api/plan") => match body::<PlanArg>(req) {
            Ok(a) => match probe::probe(t, Path::new(&a.path)).and_then(|m| plan::resolve(t, &a.settings, &m)) {
                Ok(p) => ok(p),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        (Method::Post, "/api/estimate") => match body::<PlanArg>(req) {
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
        (Method::Post, "/api/preview") => match body::<PlanArg>(req) {
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
        (Method::Post, "/api/enqueue") => match body::<JobRequest>(req) {
            Ok(r) => match ctx.engine.enqueue(r) {
                Ok(s) => ok(s),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        (Method::Post, "/api/cancel") => match body::<IdArg>(req) {
            Ok(a) => ok(ctx.engine.cancel(a.id)),
            Err(e) => err(e),
        },
        (Method::Post, "/api/cancel-all") => {
            ctx.engine.cancel_all();
            ok(true)
        }
        (Method::Post, "/api/remove") => match body::<IdArg>(req) {
            Ok(a) => ok(ctx.engine.remove(a.id)),
            Err(e) => err(e),
        },
        (Method::Post, "/api/retry") => match body::<IdArg>(req) {
            Ok(a) => match ctx.engine.retry(a.id) {
                Ok(s) => ok(s),
                Err(e) => err(e),
            },
            Err(e) => err(e),
        },
        (Method::Post, "/api/upload") => {
            let name = query_param(&url, "name").unwrap_or_else(|| "upload.bin".into());
            let safe: String = name
                .chars()
                .map(|c| if c.is_alphanumeric() || " ._-()".contains(c) { c } else { '_' })
                .collect();
            let dest = ctx.uploads.join(safe.trim_start_matches('.'));
            let mut data = Vec::new();
            if let Err(e) = req.as_reader().read_to_end(&mut data) {
                return err(e);
            }
            match std::fs::write(&dest, data) {
                Ok(()) => ok(json!({ "path": dest.to_string_lossy() })),
                Err(e) => err(e),
            }
        }
        _ => (404, json!({ "error": "not found" })),
    }
}

fn handle(ctx: Arc<Ctx>, mut req: Request) {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("/").to_string();
    if path == "/api/events" {
        let (tx, rx) = mpsc::channel();
        ctx.subs.lock().unwrap().push(tx);
        let initial = serde_json::to_string(&json!({ "type": "snapshot", "jobs": ctx.engine.jobs() })).unwrap();
        let reader = SseReader { rx, buf: Cursor::new(format!("data: {initial}\n\n").into_bytes()) };
        let resp = Response::new(
            StatusCode(200),
            vec![header("Content-Type", "text/event-stream"), header("Cache-Control", "no-store")],
            reader,
            None,
            None,
        );
        let _ = req.respond(resp);
        return;
    }
    if path == "/api/file" {
        let Some(p) = query_param(&url, "path") else {
            let _ = req.respond(json_response(400, &json!({"error": "path required"})));
            return;
        };
        let p = PathBuf::from(p);
        match std::fs::File::open(&p) {
            Ok(f) => {
                let resp = Response::from_file(f).with_header(header("Content-Type", mime(&p)));
                let _ = req.respond(resp);
            }
            Err(e) => {
                let _ = req.respond(json_response(404, &json!({"error": e.to_string()})));
            }
        }
        return;
    }
    if path.starts_with("/api/") {
        let (status, v) = api(&ctx, &mut req);
        let _ = req.respond(json_response(status, &v));
        return;
    }
    // Static UI
    if let Some(root) = &ctx.ui {
        let rel = path.trim_start_matches('/');
        let mut file = root.join(if rel.is_empty() { "index.html" } else { rel });
        if rel.contains("..") || !file.is_file() {
            file = root.join("index.html");
        }
        if let Ok(f) = std::fs::File::open(&file) {
            let _ = req.respond(Response::from_file(f).with_header(header("Content-Type", mime(&file))));
            return;
        }
    }
    let _ = req.respond(Response::from_string("Not found").with_status_code(StatusCode(404)));
}

pub fn serve(tools: Tools, port: u16, ui: Option<PathBuf>, uploads: Option<PathBuf>) -> Result<(), String> {
    let uploads = uploads.unwrap_or_else(|| std::env::temp_dir().join("pocketreel-uploads"));
    std::fs::create_dir_all(uploads.join("converted")).map_err(|e| e.to_string())?;
    let subs: Subscribers = Arc::new(Mutex::new(Vec::new()));
    let sink_subs = subs.clone();
    let engine = Engine::new(tools, move |ev| {
        let msg = serde_json::to_string(&ev).unwrap();
        sink_subs.lock().unwrap().retain(|s| s.send(msg.clone()).is_ok());
    });
    let server = Server::http(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    eprintln!("Pocket Reel bridge on http://127.0.0.1:{port}  (uploads: {})", uploads.display());
    let ctx = Arc::new(Ctx { engine, subs, ui, uploads });
    for req in server.incoming_requests() {
        let ctx = ctx.clone();
        std::thread::spawn(move || handle(ctx, req));
    }
    Ok(())
}
