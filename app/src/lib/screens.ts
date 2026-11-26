// Screen sizes the original converter offers for its AVI output (AVISIZE in AmvTransform.ini).
export const CLASSIC_SCREENS = [
  "96x64",
  "128x96",
  "128x128",
  "160x120",
  "160x128",
  "176x128",
  "176x144",
  "192x160",
  "208x120",
  "208x160",
  "208x176",
  "320x240",
];

/** Download quality for links. Small players don't gain from more than 360p,
 * and lower quality downloads much faster. */
export const QUALITIES = [
  { value: 360, label: "360p (recommended)", short: "360p" },
  { value: 480, label: "480p", short: "480p" },
  { value: 720, label: "720p", short: "720p" },
  { value: 1080, label: "1080p", short: "1080p" },
  { value: 0, label: "Best available", short: "Best" },
];
