export function bytes(n: number | null | undefined): string {
  if (n == null || !isFinite(n)) return "–";
  if (n < 1000) return `${Math.round(n)} B`;
  if (n < 1e6) return `${Math.round(n / 1e3)} KB`;
  if (n < 1e9) return `${(n / 1e6).toFixed(n < 1e7 ? 1 : 0)} MB`;
  return `${(n / 1e9).toFixed(2)} GB`;
}

/** Spoken form for screen readers: "13.4 megabytes". */
export function bytesSpoken(n: number | null | undefined): string {
  if (n == null) return "unknown size";
  if (n < 1e6) return `${Math.max(1, Math.round(n / 1e3))} kilobytes`;
  if (n < 1e9) return `${(n / 1e6).toFixed(1)} megabytes`;
  return `${(n / 1e9).toFixed(2)} gigabytes`;
}

export function clock(seconds: number | null | undefined): string {
  if (seconds == null || !isFinite(seconds)) return "–:––";
  const s = Math.max(0, Math.round(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const mm = h ? String(m).padStart(2, "0") : String(m);
  return `${h ? h + ":" : ""}${mm}:${String(sec).padStart(2, "0")}`;
}

/** Precise clock with tenths, for trim fields. */
export function clockPrecise(seconds: number): string {
  const s = Math.max(0, seconds);
  const whole = Math.floor(s);
  const tenth = Math.floor((s - whole) * 10);
  return `${clock(whole)}.${tenth}`;
}

/** Parse "1:23.4", "83.4" or "1:02:03" into seconds. */
export function parseClock(text: string): number | null {
  const t = text.trim();
  if (!t) return null;
  const parts = t.split(":").map((p) => p.trim());
  if (parts.some((p) => p === "" || isNaN(Number(p)))) return null;
  let s = 0;
  for (const p of parts) s = s * 60 + Number(p);
  return s >= 0 ? s : null;
}

export function remaining(seconds: number | null | undefined): string {
  if (seconds == null || !isFinite(seconds)) return "Estimating time…";
  if (seconds < 5) return "Almost done";
  if (seconds < 60) return `About ${Math.max(5, Math.round(seconds / 5) * 5)} seconds left`;
  const m = Math.round(seconds / 60);
  if (m < 60) return `About ${m} minute${m === 1 ? "" : "s"} left`;
  const h = Math.floor(m / 60);
  const mm = m % 60;
  return `About ${h} hour${h === 1 ? "" : "s"}${mm ? ` ${mm} min` : ""} left`;
}

export function durationWords(seconds: number): string {
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const parts: string[] = [];
  if (h) parts.push(`${h} hour${h === 1 ? "" : "s"}`);
  if (m) parts.push(`${m} minute${m === 1 ? "" : "s"}`);
  if (sec || !parts.length) parts.push(`${sec} second${sec === 1 ? "" : "s"}`);
  return parts.join(" ");
}

export function fps(v: number): string {
  if (!v) return "–";
  const r = Math.round(v * 100) / 100;
  return Number.isInteger(r) ? String(r) : r.toFixed(2).replace(/0$/, "");
}

export function kbps(v: number): string {
  return v >= 1000 ? `${(v / 1000).toFixed(v % 1000 === 0 ? 0 : 1)} Mbps` : `${v} kbps`;
}

export function hz(v: number): string {
  return `${(v / 1000).toFixed(v % 1000 === 0 ? 0 : 1)} kHz`;
}

export function basename(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export function dirname(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i > 0 ? path.slice(0, i) : path;
}

export function codecName(c: string): string {
  const map: Record<string, string> = {
    h264: "H.264",
    hevc: "HEVC",
    vp9: "VP9",
    vp8: "VP8",
    av1: "AV1",
    mpeg4: "MPEG-4",
    mjpeg: "MJPEG",
    aac: "AAC",
    mp3: "MP3",
    opus: "Opus",
    vorbis: "Vorbis",
    flac: "FLAC",
    ac3: "AC-3",
    eac3: "E-AC-3",
    pcm_s16le: "PCM",
    wmv2: "WMV",
    wmav2: "WMA",
    adpcm_ima_wav: "IMA ADPCM",
    amv: "AMV",
  };
  return map[c] ?? c.toUpperCase();
}

/** Short time-left label: "12 sec left", "3 min left", "1 h 5 min left". */
export function timeLeft(seconds: number | null | undefined): string {
  if (seconds == null || !isFinite(seconds)) return "Working out time…";
  if (seconds < 3) return "Almost done";
  if (seconds < 60) return `${Math.max(3, Math.round(seconds))} sec left`;
  const m = Math.round(seconds / 60);
  if (m < 60) return `${m} min left`;
  const h = Math.floor(m / 60);
  return `${h} h ${m % 60} min left`;
}

/** "1.2 MB/s" */
export function speed(bytesPerSecond: number | null | undefined): string {
  return bytesPerSecond ? `${bytes(bytesPerSecond)}/s` : "";
}
