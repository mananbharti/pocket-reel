// Mirrors the serde types of crates/engine.

export type Fit = "stretch" | "letterbox" | "crop" | "contain";
export type Category = "classic" | "modern" | "audio";
export type VideoCodec = "classic-mjpeg" | "amv" | "h264" | "xvid" | "vp9";
export type AudioCodec = "classic-adpcm" | "amv-adpcm" | "aac" | "mp3" | "opus" | "pcm" | "flac";

export interface SizeOption {
  width: number;
  height: number;
}

export interface FpsOption {
  value: number;
  actual: number;
  label: string;
}

export interface VideoPreset {
  codec: VideoCodec;
  width: number;
  height: number;
  sizes: SizeOption[];
  customSize: boolean;
  fit: Fit;
  fits: Fit[];
  fps: number | null;
  fpsOptions: FpsOption[];
  customFps: boolean;
  bitrateKbps: number | null;
  bitrateMin: number;
  bitrateMax: number;
  upscale: boolean;
}

export interface AudioPreset {
  codec: AudioCodec;
  bitrateKbps: number | null;
  bitrateOptions: number[];
  sampleRate: number;
  sampleRates: number[];
  channels: number;
  editable: boolean;
}

export interface Preset {
  id: string;
  name: string;
  category: Category;
  description: string;
  formatLabel: string;
  extension: string;
  video: VideoPreset | null;
  audio: AudioPreset | null;
  requires: string[];
  available: boolean;
  unavailableReason: string | null;
  original: boolean;
}

export interface Settings {
  preset: string;
  width?: number | null;
  height?: number | null;
  fit?: Fit | null;
  fps?: number | null;
  videoBitrateKbps?: number | null;
  audioBitrateKbps?: number | null;
  sampleRate?: number | null;
  channels?: number | null;
  trimStart?: number | null;
  trimEnd?: number | null;
}

export interface VideoInfo {
  codec: string;
  width: number;
  height: number;
  displayWidth: number;
  displayHeight: number;
  fps: number;
  rotation: number;
  bitrate: number | null;
}

export interface AudioInfo {
  codec: string;
  sampleRate: number;
  channels: number;
  bitrate: number | null;
}

export interface MediaInfo {
  path: string;
  fileName: string;
  sizeBytes: number;
  duration: number;
  format: string;
  bitrate: number | null;
  video: VideoInfo | null;
  audio: AudioInfo | null;
  title: string | null;
}

export interface VideoPlan {
  codec: VideoCodec;
  encoder: string;
  width: number;
  height: number;
  innerWidth: number;
  innerHeight: number;
  fit: Fit;
  fpsRational: [number, number] | null;
  fps: number;
  bitrateKbps: number | null;
  samplesPerBlock: number | null;
}

export interface AudioPlan {
  codec: AudioCodec;
  encoder: string;
  bitrateKbps: number | null;
  sampleRate: number;
  channels: number;
}

export interface Plan {
  presetId: string;
  presetName: string;
  extension: string;
  classic: boolean;
  start: number;
  duration: number;
  video: VideoPlan | null;
  audio: AudioPlan | null;
  sourceHasAudio: boolean;
}

export type EstimateMethod = "exact" | "bitrate" | "sampled" | "approximate";

export interface Estimate {
  bytes: number;
  low: number;
  high: number;
  method: EstimateMethod;
}

export interface Progress {
  fraction: number;
  /** True while a link is being downloaded, before converting. */
  downloading: boolean;
  mediaSeconds: number;
  speed: number;
  etaSeconds: number | null;
  elapsedSeconds: number;
  bytes: number;
}

export type JobState = "queued" | "running" | "done" | "failed" | "canceled";

export interface JobSnapshot {
  id: number;
  input: string;
  fileName: string;
  presetId: string;
  presetName: string;
  state: JobState;
  progress: Progress | null;
  output: string;
  outputBytes: number | null;
  error: string | null;
  plan: Plan | null;
  estimate: Estimate | null;
  duration: number;
  url: string | null;
}

export interface JobRequest {
  input: string;
  settings: Settings;
  outputDir?: string | null;
  overwrite?: boolean;
  url?: string | null;
  title?: string | null;
  duration?: number | null;
}

export interface RemoteInfo {
  url: string;
  id: string;
  title: string;
  duration: number | null;
  thumbnail: string | null;
  uploader: string | null;
}

export interface Image {
  width: number;
  height: number;
  dataUrl: string;
}

export type EngineEvent =
  | { type: "job"; job: JobSnapshot }
  | { type: "progress"; id: number; progress: Progress }
  | { type: "removed"; id: number }
  | { type: "snapshot"; jobs: JobSnapshot[] };

export type Platform = "linux" | "windows" | "macos" | "android" | "ios" | "web";

export interface AppInfo {
  platform: Platform;
  ffmpeg: string;
  defaultOutputDir: string | null;
  version: string;
  /** Present when FFmpeg could not be started. */
  engineError?: string | null;
  /** Pasting and sharing video links works. */
  downloads?: boolean;
}
