// One interface, two transports: Tauri IPC in the app, HTTP + SSE when the UI
// runs in a normal browser against `pocketreel serve` during development.

import type {
  AppInfo,
  EngineEvent,
  Estimate,
  Image,
  JobRequest,
  JobSnapshot,
  MediaInfo,
  Plan,
  Preset,
  Settings,
} from "./types";

export const MEDIA_EXTENSIONS = [
  "mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv", "flv", "3gp", "3g2", "mpg", "mpeg", "ts", "mts", "m2ts",
  "vob", "ogv", "asf", "rm", "rmvb", "f4v", "dat", "amv", "mp3", "m4a", "aac", "wav", "flac", "ogg", "opus", "wma",
];

export interface DropHandlers {
  over(active: boolean): void;
  drop(paths: string[]): void;
}

export interface Api {
  info(): Promise<AppInfo>;
  presets(): Promise<Preset[]>;
  probe(path: string): Promise<MediaInfo>;
  thumbnail(path: string, at: number | null, maxWidth: number, maxHeight: number): Promise<Image>;
  plan(path: string, settings: Settings): Promise<Plan>;
  estimate(path: string, settings: Settings): Promise<Estimate>;
  preview(path: string, settings: Settings, at: number | null): Promise<Image>;
  enqueue(request: JobRequest): Promise<JobSnapshot>;
  cancel(id: number): Promise<boolean>;
  cancelAll(): Promise<void>;
  remove(id: number): Promise<boolean>;
  retry(id: number): Promise<JobSnapshot>;
  jobs(): Promise<JobSnapshot[]>;
  setConcurrency(n: number): Promise<void>;
  onEvent(cb: (e: EngineEvent) => void): () => void;
  pickFiles(): Promise<string[]>;
  pickFolder(): Promise<string | null>;
  reveal(path: string): Promise<void>;
  open(path: string): Promise<void>;
  onDrop(handlers: DropHandlers): () => void;
  /** Where outputs end up when no folder is chosen (shown in the UI). */
  readonly supportsFolders: boolean;
}

export class ApiError extends Error {}

function errText(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

// ---------------------------------------------------------------- Tauri

async function tauriApi(): Promise<Api> {
  const core = await import("@tauri-apps/api/core");
  const event = await import("@tauri-apps/api/event");
  const dialog = await import("@tauri-apps/plugin-dialog");
  const opener = await import("@tauri-apps/plugin-opener");
  const webview = await import("@tauri-apps/api/webview");

  const call = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    try {
      return await core.invoke<T>(cmd, args);
    } catch (e) {
      throw new ApiError(errText(e));
    }
  };

  const info = await call<AppInfo>("app_info");
  const mobile = info.platform === "android" || info.platform === "ios";

  return {
    supportsFolders: !mobile,
    info: async () => info,
    presets: () => call("presets"),
    probe: (path) => call("probe", { path }),
    thumbnail: (path, at, maxWidth, maxHeight) => call("thumbnail", { path, at, maxWidth, maxHeight }),
    plan: (path, settings) => call("plan", { path, settings }),
    estimate: (path, settings) => call("estimate", { path, settings }),
    preview: (path, settings, at) => call("preview", { path, settings, at }),
    enqueue: (request) => call("enqueue", { request }),
    cancel: (id) => call("cancel", { id }),
    cancelAll: () => call("cancel_all"),
    remove: (id) => call("remove", { id }),
    retry: (id) => call("retry", { id }),
    jobs: () => call("jobs"),
    setConcurrency: (n) => call("set_concurrency", { n }),
    onEvent(cb) {
      let un: (() => void) | null = null;
      let disposed = false;
      event.listen<EngineEvent>("engine-event", (e) => cb(e.payload)).then((u) => {
        if (disposed) u();
        else un = u;
      });
      return () => {
        disposed = true;
        un?.();
      };
    },
    async pickFiles() {
      const picked = await dialog.open({
        multiple: true,
        title: "Choose videos",
        filters: mobile ? undefined : [{ name: "Video and audio", extensions: MEDIA_EXTENSIONS }],
      });
      const list = picked == null ? [] : Array.isArray(picked) ? picked : [picked];
      if (!mobile) return list;
      // Android hands out content:// URIs; copy them into app storage first.
      return call<string[]>("import_uris", { uris: list });
    },
    async pickFolder() {
      const picked = await dialog.open({ directory: true, title: "Choose output folder" });
      return typeof picked === "string" ? picked : null;
    },
    async reveal(path) {
      await opener.revealItemInDir(path);
    },
    async open(path) {
      await opener.openPath(path);
    },
    onDrop(h) {
      let un: (() => void) | null = null;
      let disposed = false;
      webview
        .getCurrentWebview()
        .onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") h.over(true);
          else if (p.type === "leave") h.over(false);
          else if (p.type === "drop") {
            h.over(false);
            h.drop(p.paths);
          }
        })
        .then((u) => {
          if (disposed) u();
          else un = u;
        });
      return () => {
        disposed = true;
        un?.();
      };
    },
  };
}

// ---------------------------------------------------------------- HTTP (browser development)

function httpApi(): Api {
  const post = async <T>(url: string, body?: unknown): Promise<T> => {
    const r = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: body === undefined ? "{}" : JSON.stringify(body),
    });
    const j = await r.json();
    if (!r.ok) throw new ApiError(j?.error ?? r.statusText);
    return j as T;
  };
  const get = async <T>(url: string): Promise<T> => {
    const r = await fetch(url);
    const j = await r.json();
    if (!r.ok) throw new ApiError(j?.error ?? r.statusText);
    return j as T;
  };
  const upload = async (files: File[]): Promise<string[]> => {
    const out: string[] = [];
    for (const f of files) {
      const r = await fetch(`/api/upload?name=${encodeURIComponent(f.name)}`, { method: "POST", body: f });
      const j = await r.json();
      if (!r.ok) throw new ApiError(j?.error ?? "Upload failed");
      out.push(j.path);
    }
    return out;
  };
  const accept = MEDIA_EXTENSIONS.map((e) => "." + e).join(",");

  return {
    supportsFolders: false,
    async info() {
      const i = await get<{ ffmpeg: string; defaultOutputDir: string }>("/api/info");
      return { platform: "web", ffmpeg: i.ffmpeg, defaultOutputDir: i.defaultOutputDir, version: "1.0.0" };
    },
    presets: () => get("/api/presets"),
    probe: (path) => post("/api/probe", { path }),
    thumbnail: (path, at, maxWidth, maxHeight) => post("/api/thumbnail", { path, at, maxWidth, maxHeight }),
    plan: (path, settings) => post("/api/plan", { path, settings }),
    estimate: (path, settings) => post("/api/estimate", { path, settings }),
    preview: (path, settings, at) => post("/api/preview", { path, settings, at }),
    enqueue: (request) => post("/api/enqueue", request),
    cancel: (id) => post("/api/cancel", { id }),
    cancelAll: async () => {
      await post("/api/cancel-all");
    },
    remove: (id) => post("/api/remove", { id }),
    retry: (id) => post("/api/retry", { id }),
    jobs: () => get("/api/jobs"),
    setConcurrency: async () => {},
    onEvent(cb) {
      const es = new EventSource("/api/events");
      es.onmessage = (m) => cb(JSON.parse(m.data));
      return () => es.close();
    },
    pickFiles() {
      return new Promise((resolve, reject) => {
        const input = document.createElement("input");
        input.type = "file";
        input.multiple = true;
        input.accept = accept;
        input.onchange = () => upload(Array.from(input.files ?? [])).then(resolve, reject);
        input.click();
      });
    },
    pickFolder: async () => null,
    reveal: async (path) => {
      window.open(`/api/file?path=${encodeURIComponent(path)}`, "_blank");
    },
    open: async (path) => {
      window.open(`/api/file?path=${encodeURIComponent(path)}`, "_blank");
    },
    onDrop(h) {
      let depth = 0;
      const enter = (e: DragEvent) => {
        if (!e.dataTransfer?.types.includes("Files")) return;
        e.preventDefault();
        depth++;
        h.over(true);
      };
      const over = (e: DragEvent) => {
        if (e.dataTransfer?.types.includes("Files")) e.preventDefault();
      };
      const leave = () => {
        depth = Math.max(0, depth - 1);
        if (depth === 0) h.over(false);
      };
      const drop = (e: DragEvent) => {
        e.preventDefault();
        depth = 0;
        h.over(false);
        const files = Array.from(e.dataTransfer?.files ?? []);
        if (files.length) upload(files).then(h.drop);
      };
      window.addEventListener("dragenter", enter);
      window.addEventListener("dragover", over);
      window.addEventListener("dragleave", leave);
      window.addEventListener("drop", drop);
      return () => {
        window.removeEventListener("dragenter", enter);
        window.removeEventListener("dragover", over);
        window.removeEventListener("dragleave", leave);
        window.removeEventListener("drop", drop);
      };
    },
  };
}

export async function createApi(): Promise<Api> {
  const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  return inTauri ? tauriApi() : httpApi();
}
