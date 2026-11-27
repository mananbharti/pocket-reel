// In-app updates: look for a newer release at launch (and every few hours
// while the app stays open), then download and install it on request.

import type { Api } from "./api";
import type { InstallOutcome, UpdateInfo } from "./types";

const DISMISS_KEY = "pocketreel.update.dismissed";
const RECHECK_MS = 6 * 60 * 60 * 1000;

export type UpdatePhase = "idle" | "available" | "download" | "verify" | "install" | "done" | "error";

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* private mode: the banner just comes back next launch */
  }
}

export class Updates {
  info = $state<UpdateInfo | null>(null);
  phase = $state<UpdatePhase>("idle");
  downloaded = $state(0);
  total = $state<number | null>(null);
  error = $state<string | null>(null);
  outcome = $state<InstallOutcome | null>(null);
  dismissed = $state<string | null>(read(DISMISS_KEY));

  private api: Api | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;

  /** 0..1 while downloading, null when the size is unknown. */
  fraction = $derived(this.total ? Math.min(1, this.downloaded / this.total) : null);
  /** Show the banner: an update exists and wasn't put off (or work is under way). */
  visible = $derived(
    this.info != null && (this.phase !== "available" || this.dismissed !== this.info.version),
  );
  working = $derived(this.phase === "download" || this.phase === "verify" || this.phase === "install");

  start(api: Api) {
    this.api = api;
    api.onUpdateProgress((p) => {
      if (!this.working) return;
      this.phase = p.stage;
      this.downloaded = p.downloaded;
      this.total = p.total;
    });
    // Give startup a moment before going online.
    setTimeout(() => this.check(), 2500);
    this.timer ??= setInterval(() => this.check(), RECHECK_MS);
  }

  async check() {
    if (!this.api || this.working || this.phase === "done") return;
    try {
      const info = await this.api.checkUpdate();
      if (info) {
        this.info = info;
        if (this.phase === "idle" || this.phase === "error") this.phase = "available";
      }
    } catch {
      // Offline or GitHub unreachable: try again later, quietly.
    }
  }

  later() {
    if (!this.info) return;
    this.dismissed = this.info.version;
    write(DISMISS_KEY, this.info.version);
    if (this.phase === "error") this.phase = "available";
  }

  async install() {
    if (!this.api || !this.info || this.working) return;
    this.error = null;
    this.downloaded = 0;
    this.total = this.info.size || null;
    this.phase = "download";
    try {
      this.outcome = await this.api.installUpdate();
      this.phase = "done";
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.phase = "error";
    }
  }

  whatsNew() {
    this.api?.openReleasePage().catch(() => {});
  }
}

export const updates = new Updates();
