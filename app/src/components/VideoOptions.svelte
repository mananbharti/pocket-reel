<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { fps as fmtFps, kbps } from "../lib/format";
  import type { Fit } from "../lib/types";
  import ChipGroup from "./ChipGroup.svelte";
  import Segmented from "./Segmented.svelte";

  const p = $derived(app.preset!);
  const v = $derived(p.video!);
  const eff = $derived(app.effective);

  const FIT_LABEL: Record<Fit, string> = {
    stretch: "Stretch",
    letterbox: "Fit",
    crop: "Fill",
    contain: "Keep shape",
  };
  const FIT_HINT: Record<Fit, string> = {
    stretch: "Fills the screen exactly, like the original converter. Wide videos look squeezed.",
    letterbox: "Shows the whole picture with black bars where the shape differs.",
    crop: "Fills the screen by trimming the edges. Nothing is squeezed.",
    contain: "Keeps the original proportions within this size and never enlarges small videos.",
  };

  function sizeLabel(w: number, h: number): string | undefined {
    if (p.category !== "modern") return undefined;
    if (h === 2160) return "4K";
    return `${h}p`;
  }

  const sizeOptions = $derived([
    ...v.sizes.map((s) => ({ value: `${s.width}x${s.height}`, label: `${s.width} × ${s.height}`, sub: sizeLabel(s.width, s.height) })),
    ...(v.customSize ? [{ value: "custom", label: "Custom" }] : []),
  ]);
  let customMode = $state(false);
  const sizeValue = $derived.by(() => {
    const key = `${eff.width}x${eff.height}`;
    if (customMode) return "custom";
    return v.sizes.some((s) => `${s.width}x${s.height}` === key) ? key : v.customSize ? "custom" : null;
  });

  // Reset custom mode when the preset changes.
  let lastPreset = "";
  $effect(() => {
    if (p.id !== lastPreset) {
      lastPreset = p.id;
      customMode = false;
    }
  });

  function pickSize(value: string) {
    if (value === "custom") {
      customMode = true;
      return;
    }
    customMode = false;
    const [w, h] = value.split("x").map(Number);
    app.update({ width: w, height: h });
  }

  function setDim(which: "width" | "height", text: string) {
    const n = Math.round(Number(text));
    if (!Number.isFinite(n) || n < 16 || n > 7680) return;
    app.update({ [which]: n + (n % 2) });
  }

  const fpsOptions = $derived([
    ...(v.fps == null ? [{ value: 0, label: "Source" }] : []),
    ...v.fpsOptions.map((o) => ({
      value: o.value,
      label: v.codec === "classic-mjpeg" ? `${fmtFps(o.actual)} fps` : fmtFps(o.value),
      hint: o.label,
    })),
  ]);

  // Logarithmic bitrate slider.
  const RANGE = 1000;
  const toPos = (k: number) => Math.round((Math.log(k / v.bitrateMin) / Math.log(v.bitrateMax / v.bitrateMin)) * RANGE);
  const fromPos = (pos: number) => {
    const raw = v.bitrateMin * Math.pow(v.bitrateMax / v.bitrateMin, pos / RANGE);
    const step = raw < 1000 ? 25 : raw < 5000 ? 100 : 500;
    return Math.round(raw / step) * step;
  };

  const plan = $derived(app.selected?.plan?.video ?? null);
  const fitHelp = $derived(FIT_HINT[eff.fit]);
</script>

<section class="group" aria-labelledby="video-title">
  <h3 id="video-title">Video</h3>

  <div class="field">
    <span class="label" id="size-label">{eff.fit === "contain" ? "Maximum size" : "Frame size"}</span>
    <ChipGroup label="Frame size" options={sizeOptions} value={sizeValue} onchange={pickSize} />
    {#if sizeValue === "custom"}
      <div class="custom">
        <label>
          <span>Width</span>
          <input type="number" inputmode="numeric" min="16" max="7680" step="2" value={eff.width} onchange={(e) => setDim("width", e.currentTarget.value)} />
        </label>
        <span class="times" aria-hidden="true">×</span>
        <label>
          <span>Height</span>
          <input type="number" inputmode="numeric" min="16" max="4320" step="2" value={eff.height} onchange={(e) => setDim("height", e.currentTarget.value)} />
        </label>
        <span class="unit">pixels</span>
      </div>
    {/if}
  </div>

  {#if v.fits.length > 1}
    <div class="field">
      <span class="label">Picture</span>
      <Segmented
        label="How the picture fills the frame"
        options={v.fits.map((f) => ({ value: f, label: FIT_LABEL[f] }))}
        value={eff.fit}
        onchange={(f) => app.update({ fit: f })}
      />
      <p class="help">{fitHelp}</p>
    </div>
  {/if}

  <div class="field">
    <span class="label">Frame rate</span>
    <Segmented
      label="Frame rate"
      options={fpsOptions}
      value={v.fps == null && !app.settings.fps ? 0 : eff.fps}
      onchange={(f) => app.update({ fps: f })}
    />
    {#if v.codec === "classic-mjpeg"}
      <p class="help">One audio block per frame, exactly like the original tool: High plays 21.68 frames per second, Low 14.42.</p>
    {:else if plan}
      <p class="help num">This video: {fmtFps(plan.fps)} fps</p>
    {/if}
  </div>

  {#if v.bitrateKbps != null}
    <div class="field">
      <label class="label row" for="vbr">
        <span>Video bitrate</span>
        <span class="value num">{kbps(eff.videoBitrate)}</span>
      </label>
      <input
        id="vbr"
        class="slider"
        type="range"
        min="0"
        max={RANGE}
        value={toPos(eff.videoBitrate)}
        aria-valuetext={kbps(eff.videoBitrate)}
        style="--fill: {(toPos(eff.videoBitrate) / RANGE) * 100}%"
        oninput={(e) => app.update({ videoBitrateKbps: fromPos(Number(e.currentTarget.value)) })}
      />
      <div class="scale num" aria-hidden="true"><span>Smaller file</span><span>Higher quality</span></div>
    </div>
  {/if}
</section>

<style>
  .custom {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    margin-top: 10px;
    animation: rise var(--t-med) var(--spring-soft);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .custom label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-2);
  }
  .custom input {
    width: 92px;
  }
  .times {
    padding-bottom: 8px;
    color: var(--text-3);
  }
  .unit {
    padding-bottom: 9px;
    font-size: 12.5px;
    color: var(--text-3);
  }
</style>
