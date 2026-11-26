<script lang="ts">
  import { app, type Item } from "../lib/store.svelte";
  import { bytes, clock, fps, hz } from "../lib/format";
  import Icon from "./Icon.svelte";
  import TrimControl from "./TrimControl.svelte";

  let { item }: { item: Item } = $props();
  const plan = $derived(item.plan);
  const v = $derived(plan?.video ?? null);
  const a = $derived(plan?.audio ?? null);
  const dur = $derived(item.media?.duration ?? 0);
  const start = $derived(item.trimStart ?? 0);
  const end = $derived(item.trimEnd ?? dur);
  const at = $derived(app.previewAt ?? start + Math.min((end - start) * 0.2, 8));
  const img = $derived(app.preview);
  // Show tiny outputs with crisp pixels, the way the player will.
  const pixelated = $derived(img ? img.width <= 240 : false);
  let showTrim = $state(false);
  const trimmed = $derived(item.trimStart != null || item.trimEnd != null);
</script>

<section class="group" aria-labelledby="sel-title">
  <h3 id="sel-title">
    <span class="title">Output</span>
    <span class="file" title={item.name}>{item.name}</span>
  </h3>

  {#if item.status === "error"}
    <p class="help err">{item.error}</p>
  {:else if item.planError}
    <p class="help err" role="alert">{item.planError}</p>
  {:else if v && app.preset?.video}
    <figure class="stage" class:loading={app.previewLoading}>
      {#if img}
        <img
          src={img.dataUrl}
          alt="Preview of {item.name} at {img.width} by {img.height} pixels"
          class:pixelated
          style="aspect-ratio: {img.width} / {img.height}"
        />
      {:else if app.previewError}
        <div class="ph err">{app.previewError}</div>
      {:else}
        <div class="ph" style="aspect-ratio: {v.width} / {v.height}"></div>
      {/if}
      <span class="spinner" aria-hidden="true"></span>
    </figure>
    <label class="scrub">
      <span class="visually-hidden">Preview frame time</span>
      <input
        class="slider"
        type="range"
        min={start}
        max={Math.max(start + 0.1, end - 0.05)}
        step="0.1"
        value={at}
        aria-valuetext={clock(at)}
        style="--fill: {((at - start) / Math.max(0.1, end - start)) * 100}%"
        oninput={(e) => app.setPreviewAt(Number(e.currentTarget.value))}
      />
      <span class="t num" aria-hidden="true">{clock(at)}</span>
    </label>
    <dl class="facts num">
      <div><dt>Size</dt><dd>{v.width} × {v.height}</dd></div>
      <div><dt>Frame rate</dt><dd>{fps(v.fps)} fps</dd></div>
      <div><dt>Length</dt><dd>{clock(plan?.duration)}</dd></div>
      <div>
        <dt>File size</dt>
        <dd class="est">
          {#if item.estimating || !item.estimate}
            <span class="dots">Estimating</span>
          {:else}
            ≈ {bytes(item.estimate.bytes)}
          {/if}
        </dd>
      </div>
    </dl>
  {:else if a}
    <div class="audio">
      <span class="aico"><Icon name="music" size={26} stroke={1.6} /></span>
      <dl class="facts num">
        <div><dt>Format</dt><dd>{app.preset?.extension.toUpperCase()}</dd></div>
        <div><dt>Quality</dt><dd>{a.bitrateKbps ? `${a.bitrateKbps} kbps` : "Lossless"} · {hz(a.sampleRate)}</dd></div>
        <div><dt>Length</dt><dd>{clock(plan?.duration)}</dd></div>
        <div><dt>File size</dt><dd class="est">{item.estimate && !item.estimating ? `≈ ${bytes(item.estimate.bytes)}` : "Estimating"}</dd></div>
      </dl>
    </div>
  {/if}

  {#if item.status === "ready" && dur > 1}
    <div class="trimwrap">
      <button type="button" class="disclosure" aria-expanded={showTrim} onclick={() => (showTrim = !showTrim)}>
        <Icon name="scissors" size={16} />
        <span>Trim</span>
        {#if trimmed && !showTrim}<span class="pill num">{clock(start)} – {clock(end)}</span>{/if}
        <span class="chev" class:open={showTrim}><Icon name="chevronDown" size={16} /></span>
      </button>
      {#if showTrim}
        <div class="trimbody"><TrimControl {item} /></div>
      {/if}
    </div>
  {/if}
</section>

<style>
  h3 .file {
    text-transform: none;
    letter-spacing: 0;
    font-weight: 560;
    color: var(--text-3);
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 62%;
  }
  .stage {
    position: relative;
    margin: 0;
    border-radius: var(--r-md);
    overflow: hidden;
    background: var(--surface-2);
    display: grid;
    place-items: center;
    min-height: 120px;
    padding: 10px;
    border: 1px solid var(--separator);
  }
  .stage img {
    display: block;
    width: auto;
    height: auto;
    max-width: 100%;
    max-height: 260px;
    min-width: 50%;
    object-fit: contain;
    border-radius: 3px;
    box-shadow: 0 1px 8px rgba(0, 0, 0, 0.18);
    transition: opacity var(--t-med) var(--ease-out), filter var(--t-med);
    animation: fadein var(--t-slow) var(--ease-out);
  }
  @keyframes fadein {
    from {
      opacity: 0;
    }
  }
  .stage img.pixelated {
    image-rendering: pixelated;
  }
  .stage.loading img {
    opacity: 0.55;
    filter: saturate(0.6);
  }
  .ph {
    width: 100%;
    max-height: 260px;
    background: var(--surface-2);
    display: grid;
    place-items: center;
    padding: 16px;
    font-size: 12.5px;
  }
  .spinner {
    position: absolute;
    top: 10px;
    right: 10px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.35);
    border-top-color: #fff;
    opacity: 0;
    transition: opacity var(--t-fast);
    animation: spin 0.8s linear infinite;
  }
  .loading .spinner {
    opacity: 1;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .scrub {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
  }
  .t {
    font-size: 12px;
    color: var(--text-2);
    min-width: 38px;
    text-align: right;
  }
  .facts {
    margin: 10px 0 0;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }
  .facts div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  dt {
    font-size: 11.5px;
    color: var(--text-3);
    font-weight: 560;
  }
  dd {
    margin: 0;
    font-size: 13.5px;
    font-weight: 620;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  dd.est {
    color: var(--accent);
  }
  .dots::after {
    content: "";
    animation: dots 1.2s steps(4) infinite;
  }
  @keyframes dots {
    0% {
      content: "";
    }
    25% {
      content: ".";
    }
    50% {
      content: "..";
    }
    75% {
      content: "...";
    }
  }
  .audio {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 14px;
    align-items: center;
  }
  .audio .facts {
    margin: 0;
    grid-template-columns: repeat(2, 1fr);
  }
  .aico {
    width: 64px;
    height: 64px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .err {
    color: var(--danger) !important;
  }
  .trimwrap {
    margin-top: 12px;
    border-top: 1px solid var(--separator);
    padding-top: 6px;
  }
  .disclosure {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 38px;
    padding: 0 4px;
    border: 0;
    background: transparent;
    font-weight: 600;
    font-size: 13.5px;
    border-radius: var(--r-sm);
  }
  .disclosure:hover {
    background: var(--fill);
  }
  .pill {
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
    background: var(--accent-soft);
    padding: 1px 8px;
    border-radius: 999px;
  }
  .chev {
    margin-left: auto;
    color: var(--text-3);
    transition: transform var(--t-med) var(--spring);
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .trimbody {
    padding: 4px 2px 2px;
    animation: open var(--t-slow) var(--spring-soft);
  }
  @keyframes open {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  @media (max-width: 420px) {
    .facts {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
