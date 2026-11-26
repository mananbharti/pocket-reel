import type { Preset } from "./types";

export function presetIcon(p: Preset | null | undefined): string {
  if (!p) return "film";
  if (p.category === "audio") return "music";
  if (p.id.startsWith("classic")) return "tv";
  if (p.id === "dvd-xvid") return "disc";
  if (p.id === "webm") return "globe";
  if (p.id === "mp4-1080p") return "film";
  return "phone";
}

export const CATEGORY_TITLES: Record<string, string> = {
  classic: "Classic players",
  modern: "Phones, TVs and the web",
  audio: "Audio only",
};
