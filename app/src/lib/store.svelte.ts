import { ApiError, createApi, type Api } from "./api";
import { basename, bytes, bytesSpoken } from "./format";
import type {
  AppInfo,
  EngineEvent,
  Estimate,
  Fit,
  Image,
  JobSnapshot,
  MediaInfo,
  Plan,
  Preset,
  RemoteInfo,
  Settings,
} from "./types";

export type Theme = "auto" | "light" | "dark";

export interface Prefs {
  theme: Theme;
  contrast: "normal" | "high";
  motion: "system" | "reduce";
  concurrency: number;
  preset: string;
  outputDir: string | null;
  /** Player screen size, "WIDTHxHEIGHT". */
  screen: string;
}

const PREFS_KEY = "pocketreel.prefs";

function loadPrefs(): Prefs {
  const defaults: Prefs = {
    theme: "auto",
    contrast: "normal",
    motion: "system",
    concurrency: 2,
    preset: "classic-avi",
    outputDir: null,
    screen: "160x120",
  };
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(PREFS_KEY) || "{}") };
  } catch {
    return defaults;
  }
}

export interface Item {
  key: string;
  path: string;
  name: string;
  status: "probing" | "ready" | "error";
  media: MediaInfo | null;
  thumb: string | null;
  error: string | null;
  /** Set for videos added from a link; they are downloaded when converted. */
  link: RemoteInfo | null;
  isLink: boolean;
  trimStart: number | null;
  trimEnd: number | null;
  plan: Plan | null;
  planError: string | null;
  estimate: Estimate | null;
  estimating: boolean;
  jobId: number | null;
}

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  text: string;
}

let keySeq = 0;
let toastSeq = 0;

export class AppState {
  api: Api | null = null;
  ready = $state(false);
  fatal = $state<string | null>(null);
  info = $state<AppInfo | null>(null);
  presets = $state<Preset[]>([]);
  items = $state<Item[]>([]);
  jobs = $state<Record<number, JobSnapshot>>({});
  selectedKey = $state<string | null>(null);
  settings = $state<Settings>({ preset: "classic-avi" });
  prefs = $state<Prefs>(loadPrefs());
  dragActive = $state(false);
  announcement = $state("");
  toasts = $state<Toast[]>([]);
  preview = $state<Image | null>(null);
  previewLoading = $state(false);
  previewError = $state<string | null>(null);
  previewAt = $state<number | null>(null);

  private estimateGen = 0;
  private previewGen = 0;
  private estimateTimer: ReturnType<typeof setTimeout> | null = null;
  private previewTimer: ReturnType<typeof setTimeout> | null = null;

  preset = $derived(this.presets.find((p) => p.id === this.settings.preset) ?? null);
  selected = $derived(this.items.find((i) => i.key === this.selectedKey) ?? null);
  isMobile = $derived(this.info?.platform === "android" || this.info?.platform === "ios");

  /** Items that would be converted by the Convert button. */
  convertible = $derived(
    this.items.filter((i) => {
      if (i.status !== "ready" || i.planError) return false;
      const j = i.jobId != null ? this.jobs[i.jobId] : null;
      return !j || j.state === "failed" || j.state === "canceled" || j.state === "done";
    }),
  );
  /** Items not yet converted at all (preferred target of Convert). */
  pending = $derived(
    this.convertible.filter((i) => i.jobId == null || this.jobs[i.jobId]?.state !== "done"),
  );
  active = $derived(Object.values(this.jobs).filter((j) => j.state === "running" || j.state === "queued"));
  totalEstimate = $derived(
    this.pending.reduce((sum, i) => sum + (i.estimate?.bytes ?? 0), 0),
  );
  /** Links have no size until downloaded, so only show a total for files. */
  estimatesPending = $derived(
    this.pending.some((i) => i.isLink || i.estimating || (!i.estimate && !i.planError)),
  );
  canDownload = $derived(!!this.info?.downloads);

  async init() {
    this.applyPrefs();
    try {
      this.api = await createApi();
      const [info, presets] = await Promise.all([this.api.info(), this.api.presets()]);
      this.info = info;
      if (info.engineError) this.fatal = info.engineError;
      this.presets = presets;
      // One job only: the MP3 player's own AVI format at the player's screen size.
      this.settings = this.screenSettings();
      this.api.onEvent((e) => this.onEvent(e));
      this.api.onDrop({
        over: (a) => (this.dragActive = a),
        drop: (paths) => this.addPaths(paths),
      });
      const existing = await this.api.jobs();
      for (const j of existing) this.jobs[j.id] = j;
      this.api.setConcurrency(this.prefs.concurrency);
      this.ready = true;
      if (info.downloads) this.watchShares();
    } catch (e) {
      this.fatal = e instanceof Error ? e.message : String(e);
    }
  }

  // ------------------------------------------------------------ preferences

  setPref<K extends keyof Prefs>(key: K, value: Prefs[K]) {
    this.prefs[key] = value;
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(this.prefs));
    } catch {
      /* storage can be unavailable; preferences then last for this session */
    }
    this.applyPrefs();
    if (key === "concurrency") this.api?.setConcurrency(value as number);
  }

  applyPrefs() {
    const root = document.documentElement;
    const p = this.prefs;
    if (p.theme === "auto") delete root.dataset.theme;
    else root.dataset.theme = p.theme;
    if (p.contrast === "high") root.dataset.contrast = "high";
    else delete root.dataset.contrast;
    if (p.motion === "reduce") root.dataset.motion = "reduce";
    else delete root.dataset.motion;
  }

  // ------------------------------------------------------------ announcements / toasts

  announce(text: string) {
    // Clear first so repeated messages are re-read.
    this.announcement = "";
    queueMicrotask(() => (this.announcement = text));
  }

  toast(kind: Toast["kind"], text: string, ms = 4200) {
    const id = ++toastSeq;
    this.toasts = [...this.toasts, { id, kind, text }];
    setTimeout(() => this.dismissToast(id), ms);
  }

  dismissToast(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  // ------------------------------------------------------------ files

  async pickFiles() {
    if (!this.api) return;
    try {
      const paths = await this.api.pickFiles();
      if (paths.length) await this.addPaths(paths);
    } catch (e) {
      console.error("pickFiles failed", e);
      const msg = e instanceof Error ? e.message : String(e);
      this.toast("error", `Couldn't open those files: ${msg}`, 12000);
      this.announce(`Couldn't open those files. ${msg}`);
    }
  }

  async addPaths(paths: string[]) {
    const known = new Set(this.items.map((i) => i.path));
    const fresh = paths.filter((p) => !known.has(p));
    if (!fresh.length) {
      if (paths.length) this.announce("Those files are already in the list.");
      return;
    }
    const added: Item[] = fresh.map((path) => ({
      key: `i${++keySeq}`,
      path,
      name: basename(path),
      status: "probing",
      media: null,
      thumb: null,
      error: null,
      link: null,
      isLink: false,
      trimStart: null,
      trimEnd: null,
      plan: null,
      planError: null,
      estimate: null,
      estimating: false,
      jobId: null,
    }));
    this.items = [...this.items, ...added];
    if (!this.selectedKey) this.selectedKey = added[0].key;
    this.announce(`Added ${added.length} file${added.length === 1 ? "" : "s"}.`);
    await Promise.all(added.map((it) => this.loadItem(it.key)));
    this.scheduleEstimates(0);
    this.schedulePreview(0);
  }

  /** Add a video from a link (YouTube and any other site yt-dlp supports). */
  /** Add a pasted or shared link. `start` converts it as soon as its details load. */
  async addLink(text: string, start = false) {
    const api = this.api;
    const url = text.match(/https?:\/\/\S+/)?.[0];
    if (!api || !url) {
      this.toast("error", "That doesn't look like a video link.");
      return false;
    }
    if (this.items.some((i) => i.path === url)) {
      this.announce("That link is already in the list.");
      return false;
    }
    const key = `i${++keySeq}`;
    this.items = [
      ...this.items,
      {
        key,
        path: url,
        name: url.replace(/^https?:\/\/(www\.)?/, ""),
        status: "probing",
        media: null,
        thumb: null,
        error: null,
        link: null,
        isLink: true,
        trimStart: null,
        trimEnd: null,
        plan: null,
        planError: null,
        estimate: null,
        estimating: false,
        jobId: null,
      },
    ];
    this.announce("Getting the video's details.");
    try {
      const info = await api.linkInfo(url);
      const it = this.item(key);
      if (it) {
        it.link = info;
        it.name = info.title;
        it.thumb = info.thumbnail;
        it.status = "ready";
        this.announce(`Added ${info.title}.`);
        if (start) await this.convert([it]);
      }
    } catch (e) {
      const it = this.item(key);
      if (it) {
        it.status = "error";
        it.error = e instanceof Error ? e.message : String(e);
      }
    }
    return true;
  }

  /** Links shared from other apps (Android's Share menu). */
  private watchShares() {
    const check = async () => {
      try {
        const url = await this.api?.takeShared();
        if (url) await this.addLink(url, true);
      } catch {
        /* nothing shared */
      }
    };
    check();
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") check();
    });
    window.addEventListener("focus", check);
    // Sharing to the app while it is already open doesn't always refocus it.
    setInterval(() => {
      if (document.visibilityState === "visible") check();
    }, 2000);
  }

  private item(key: string): Item | undefined {
    return this.items.find((i) => i.key === key);
  }

  private async loadItem(key: string) {
    const api = this.api!;
    const it = this.item(key);
    if (!it) return;
    try {
      const media = await api.probe(it.path);
      const cur = this.item(key);
      if (!cur) return;
      cur.media = media;
      cur.name = media.fileName || cur.name;
      cur.status = "ready";
      if (media.video) {
        const at = Math.min(media.duration * 0.15, 8);
        api
          .thumbnail(it.path, at, 240, 160)
          .then((img) => {
            const c = this.item(key);
            if (c) c.thumb = img.dataUrl;
          })
          .catch(() => {});
      }
    } catch (e) {
      const cur = this.item(key);
      if (cur) {
        cur.status = "error";
        cur.error = e instanceof Error ? e.message : String(e);
      }
    }
  }

  removeItem(key: string) {
    const idx = this.items.findIndex((i) => i.key === key);
    if (idx < 0) return;
    const it = this.items[idx];
    const job = it.jobId != null ? this.jobs[it.jobId] : null;
    if (job && (job.state === "running" || job.state === "queued")) return;
    if (job) this.api?.remove(job.id);
    this.items = this.items.filter((i) => i.key !== key);
    if (this.selectedKey === key) {
      const next = this.items[Math.min(idx, this.items.length - 1)];
      this.selectedKey = next ? next.key : null;
      this.schedulePreview(0);
    }
    this.announce(`Removed ${it.name}.`);
  }

  clearFinished() {
    const keep = this.items.filter((i) => {
      const j = i.jobId != null ? this.jobs[i.jobId] : null;
      return !(j && j.state === "done");
    });
    for (const i of this.items) {
      if (!keep.includes(i) && i.jobId != null) this.api?.remove(i.jobId);
    }
    this.items = keep;
    if (this.selectedKey && !this.item(this.selectedKey)) {
      this.selectedKey = keep[0]?.key ?? null;
    }
  }

  select(key: string) {
    if (this.selectedKey === key) return;
    this.selectedKey = key;
    this.previewAt = null;
    this.schedulePreview(0);
  }

  setTrim(key: string, start: number | null, end: number | null) {
    const it = this.item(key);
    if (!it) return;
    it.trimStart = start;
    it.trimEnd = end;
    this.scheduleEstimates(250, [key]);
    if (key === this.selectedKey) this.schedulePreview(250);
  }

  // ------------------------------------------------------------ settings

  setPreset(id: string) {
    if (id === this.settings.preset) return;
    this.settings = { preset: id };
    this.setPref("preset", id);
    this.previewAt = null;
    this.scheduleEstimates(0);
    this.schedulePreview(0);
    const p = this.presets.find((x) => x.id === id);
    if (p) this.announce(`${p.name}: ${p.formatLabel}`);
  }

  update(patch: Partial<Settings>) {
    this.settings = { ...this.settings, ...patch };
    this.scheduleEstimates(220);
    this.schedulePreview(220);
  }

  /** Effective values shown in controls. */
  get effective() {
    const p = this.preset;
    const s = this.settings;
    return {
      width: s.width ?? p?.video?.width ?? 0,
      height: s.height ?? p?.video?.height ?? 0,
      fit: (s.fit ?? p?.video?.fit ?? "contain") as Fit,
      fps: s.fps ?? p?.video?.fps ?? 0,
      videoBitrate: s.videoBitrateKbps ?? p?.video?.bitrateKbps ?? 0,
      audioBitrate: s.audioBitrateKbps ?? p?.audio?.bitrateKbps ?? 0,
      sampleRate: s.sampleRate ?? p?.audio?.sampleRate ?? 0,
      channels: s.channels ?? p?.audio?.channels ?? 2,
    };
  }

  settingsFor(it: Item): Settings {
    return { ...this.settings, trimStart: it.trimStart, trimEnd: it.trimEnd };
  }

  // ------------------------------------------------------------ estimates and preview

  scheduleEstimates(delay = 220, keys?: string[]) {
    if (this.estimateTimer) clearTimeout(this.estimateTimer);
    for (const it of this.items) {
      if (!keys || keys.includes(it.key)) it.estimating = it.status === "ready" && !it.isLink;
    }
    this.estimateTimer = setTimeout(() => this.runEstimates(keys), delay);
  }

  private async runEstimates(keys?: string[]) {
    const api = this.api;
    if (!api) return;
    const gen = ++this.estimateGen;
    const targets = this.items.filter((i) => i.status === "ready" && !i.isLink && (!keys || keys.includes(i.key)));
    const queue = [...targets];
    const worker = async () => {
      while (queue.length) {
        const it = queue.shift()!;
        const settings = this.settingsFor(it);
        try {
          const plan = await api.plan(it.path, settings);
          if (gen !== this.estimateGen) return;
          const cur = this.item(it.key);
          if (!cur) continue;
          cur.plan = plan;
          cur.planError = null;
          const est = await api.estimate(it.path, settings);
          if (gen !== this.estimateGen) return;
          const cur2 = this.item(it.key);
          if (cur2) {
            cur2.estimate = est;
            cur2.estimating = false;
          }
        } catch (e) {
          if (gen !== this.estimateGen) return;
          const cur = this.item(it.key);
          if (cur) {
            cur.planError = e instanceof ApiError || e instanceof Error ? e.message : String(e);
            cur.estimate = null;
            cur.estimating = false;
          }
        }
      }
    };
    await Promise.all([worker(), worker()]);
  }

  screenSettings(): Settings {
    const [w, h] = this.prefs.screen.split("x").map(Number);
    return { preset: "classic-avi", width: w || 160, height: h || 120 };
  }

  setScreen(value: string) {
    this.setPref("screen", value);
    this.settings = this.screenSettings();
    this.scheduleEstimates(0);
    this.announce(`Player screen ${value.replace("x", " by ")}.`);
  }

  schedulePreview(_delay = 220) {
    // The simple app shows no preview; kept so callers stay unchanged.
  }

  setPreviewAt(t: number) {
    this.previewAt = t;
    this.schedulePreview(120);
  }

  private async runPreview() {
    const api = this.api;
    const it = this.selected;
    const gen = ++this.previewGen;
    if (!api || !it || it.status !== "ready" || !it.media?.video || !this.preset?.video) {
      this.preview = null;
      this.previewError = null;
      this.previewLoading = false;
      return;
    }
    this.previewLoading = true;
    try {
      const start = it.trimStart ?? 0;
      const end = it.trimEnd ?? it.media.duration;
      const at = this.previewAt ?? start + Math.min((end - start) * 0.2, 8);
      const img = await api.preview(it.path, this.settingsFor(it), at);
      if (gen !== this.previewGen) return;
      this.preview = img;
      this.previewError = null;
    } catch (e) {
      if (gen !== this.previewGen) return;
      this.previewError = e instanceof Error ? e.message : String(e);
    } finally {
      if (gen === this.previewGen) this.previewLoading = false;
    }
  }

  // ------------------------------------------------------------ conversion

  async convert(items: Item[] = this.pending.length ? this.pending : this.convertible) {
    const api = this.api;
    if (!api || !items.length) return;
    let queued = 0;
    for (const it of items) {
      try {
        const snap = await api.enqueue(
          it.isLink
            ? {
                input: it.path,
                url: it.path,
                title: it.link?.title ?? it.name,
                duration: it.link?.duration ?? null,
                settings: this.settingsFor(it),
                outputDir: this.prefs.outputDir,
              }
            : { input: it.path, settings: this.settingsFor(it), outputDir: this.prefs.outputDir },
        );
        this.jobs[snap.id] = snap;
        const cur = this.item(it.key);
        if (cur) cur.jobId = snap.id;
        queued++;
      } catch (e) {
        this.toast("error", `${it.name}: ${e instanceof Error ? e.message : e}`);
      }
    }
    if (queued) this.announce(`Converting ${queued} file${queued === 1 ? "" : "s"}.`);
  }

  cancel(jobId: number) {
    this.api?.cancel(jobId);
  }

  cancelAll() {
    this.api?.cancelAll();
  }

  async retry(it: Item) {
    if (it.jobId == null) return;
    await this.convert([it]);
  }

  private onEvent(e: EngineEvent) {
    switch (e.type) {
      case "snapshot":
        for (const j of e.jobs) this.jobs[j.id] = j;
        break;
      case "progress": {
        const j = this.jobs[e.id];
        if (j) j.progress = e.progress;
        break;
      }
      case "removed":
        delete this.jobs[e.id];
        break;
      case "job": {
        const prev = this.jobs[e.job.id];
        this.jobs[e.job.id] = e.job;
        if (prev?.state !== e.job.state) this.onStateChange(e.job);
        break;
      }
    }
  }

  private onStateChange(j: JobSnapshot) {
    const name = j.fileName;
    if (j.state === "done") {
      this.announce(`${name} converted, ${bytesSpoken(j.outputBytes)}.`);
    } else if (j.state === "failed") {
      this.announce(`${name} failed. ${j.error ?? ""}`);
      this.toast("error", `${name}: ${j.error ?? "Conversion failed"}`, 7000);
    } else if (j.state === "canceled") {
      this.announce(`${name} canceled.`);
    } else if (j.state === "running") {
      this.announce(`Converting ${name}.`);
    }
  }

  jobFor(it: Item): JobSnapshot | null {
    return it.jobId != null ? (this.jobs[it.jobId] ?? null) : null;
  }
}

export const app = new AppState();

if (import.meta.env.DEV) {
  (window as unknown as { __app: AppState }).__app = app;
}
