<script lang="ts">
  import { app, jobRemaining, type Item } from "../lib/store.svelte";
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { bytes, bytesSpoken, clock, durationWords, remaining, speed, timeLeft } from "../lib/format";
  import Icon from "./Icon.svelte";
  import ProgressBar from "./ProgressBar.svelte";

  let { item }: { item: Item } = $props();

  const job = $derived(app.jobFor(item));
  const busy = $derived(job?.state === "running" || job?.state === "queued");
  const pct = $derived(Math.floor((job?.progress?.fraction ?? 0) * 100));

  const site = $derived.by(() => {
    try {
      return new URL(item.path).hostname.replace(/^(www|m)\./, "");
    } catch {
      return "the web";
    }
  });
  const length = $derived(item.isLink ? item.link?.duration : item.media?.duration);
  const downloading = $derived(job?.state === "running" && !!job.progress?.downloading);
  const running = $derived(job?.state === "running");
  // Before the first bytes arrive (YouTube needs a few seconds to get started).
  const connecting = $derived(downloading && !job?.progress?.bytes && !job?.progress?.fraction);
  const stage = $derived(running ? (downloading ? "download" : "convert") : null);
  const stepLabel = $derived(item.isLink && running ? (downloading ? "Step 1 of 2" : "Step 2 of 2") : "");
  // Whole time left for this video: what's left to download plus converting.
  const eta = $derived(job ? jobRemaining(job) : null);

  /** Size and speed while downloading; speed versus playback while converting. */
  const detail = $derived.by(() => {
    const p = job?.progress;
    if (!p || !running) return "";
    if (downloading) {
      if (connecting) return "Finding the video…";
      const size = p.bytes ? (p.totalBytes ? `${bytes(p.bytes)} of ${bytes(p.totalBytes)}` : bytes(p.bytes)) : "Starting…";
      const rate = speed(p.bytesPerSecond);
      return rate ? `${size} · ${rate}` : size;
    }
    if (p.speed > 0) return `${p.speed >= 10 ? Math.round(p.speed) : p.speed.toFixed(1)}× playback speed`;
    return "Starting…";
  });

  // The ring on the thumbnail glides between updates.
  const ring = new Tween(0, { duration: 600, easing: cubicOut });
  $effect(() => {
    const v = running ? (job?.progress?.fraction ?? 0) : 0;
    ring.set(v, v < ring.current - 0.02 ? { duration: 0 } : undefined);
  });
  const RING = 2 * Math.PI * 17;

  const status = $derived.by(() => {
    if (item.status === "probing") return item.isLink ? "Getting video details…" : "Reading…";
    if (item.status === "error") return item.error ?? "Can't read this file";
    if (!job) {
      if (item.isLink) return `From ${site}${length ? ` · ${clock(length)}` : ""}`;
      return item.estimate && !item.estimating ? `${clock(length)} · about ${bytes(item.estimate.bytes)}` : clock(length);
    }
    if (job.state === "queued") return "Waiting to start…";
    if (connecting) return `Connecting to ${site}…`;
    if (downloading) return `Downloading ${pct}%`;
    if (job.state === "running") return `Converting ${pct}%`;
    if (job.state === "done") return `Ready for your player · ${bytes(job.outputBytes)}`;
    if (job.state === "canceled") return "Stopped";
    return job.error ?? "Couldn't convert";
  });

  const spoken = $derived.by(() => {
    const p = [item.name];
    if (length) p.push(durationWords(length));
    if (item.isLink) p.push(`from ${site}`);
    if (job?.state === "done") p.push(`ready, ${bytesSpoken(job.outputBytes)}`);
    else if (job?.state === "failed") p.push(`failed: ${job.error}`);
    else if (downloading) p.push(`downloading, ${pct} percent`);
    else if (job?.state === "running") p.push(`converting, ${pct} percent`);
    else if (item.estimate) p.push(`about ${bytesSpoken(item.estimate.bytes)}`);
    return p.join(", ");
  });

  const tone = $derived(
    item.status === "error" || job?.state === "failed" ? "bad" : job?.state === "done" ? "good" : "",
  );
</script>

<div class="row {tone}" class:running class:queued={job?.state === "queued"} role="group" aria-label={spoken}>
  <span class="thumb" aria-hidden="true">
    {#if item.thumb}
      <img src={item.thumb} alt="" />
    {:else if item.status === "probing"}
      <span class="shimmer"></span>
    {:else}
      <Icon name={item.isLink ? "globe" : "film"} size={22} stroke={1.6} />
    {/if}
    {#if running}
      <span class="busy {stage}">
        <svg viewBox="0 0 40 40" class="ring">
          <circle cx="20" cy="20" r="17" class="track" />
          <circle cx="20" cy="20" r="17" class="arc" stroke-dasharray={RING} stroke-dashoffset={RING * (1 - ring.current)} />
        </svg>
        <span class="glyph"><Icon name={downloading ? "arrowDown" : "film"} size={14} stroke={2.4} /></span>
      </span>
    {/if}
    {#if job?.state === "done"}
      <span class="tick"><Icon name="check" size={16} stroke={3} /></span>
    {/if}
  </span>

  <span class="info">
    <span class="name" title={item.isLink ? item.path : undefined}>{item.name}</span>
    <span class="status num">
      {#if stepLabel}<span class="step {stage}">{stepLabel}</span>{/if}
      {#key stage}<span class="stage-text">{status}</span>{/key}
    </span>
    {#if busy}
      <ProgressBar
        value={job?.state === "queued" || connecting ? null : (job?.progress?.fraction ?? 0)}
        {stage}
        label="{downloading ? 'Downloading' : 'Converting'} {item.name}"
        valueText={job?.state === "queued" ? "Waiting" : `${downloading ? "Downloading" : "Converting"} ${pct}%, ${remaining(eta)}`}
      />
      {#if running}
        <span class="meta num">
          <span class="detail">{detail}</span>
          <span class="left"><Icon name="clock" size={13} stroke={2} />{timeLeft(eta)}</span>
        </span>
      {/if}
    {/if}
  </span>

  <span class="tools">
    {#if busy && job}
      <button type="button" class="btn icon plain" aria-label="Stop converting {item.name}" title="Stop" onclick={() => app.cancel(job.id)}>
        <Icon name="stop" size={18} />
      </button>
    {:else}
      {#if job?.state === "done" && !app.isMobile}
        <button type="button" class="btn plain show" onclick={() => app.api?.reveal(job.output)}>
          <Icon name="folder" size={17} /> Show
        </button>
      {:else if job?.state === "failed" || job?.state === "canceled"}
        <button type="button" class="btn plain show" onclick={() => app.retry(item)}>
          <Icon name="retry" size={17} /> Try again
        </button>
      {/if}
      <button type="button" class="btn icon plain" aria-label="Remove {item.name}" title="Remove" onclick={() => app.removeItem(item.key)}>
        <Icon name="x" size={17} />
      </button>
    {/if}
  </span>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 14px;
    padding: 12px 10px 12px 12px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: var(--border-width) solid var(--separator);
    box-shadow: var(--shadow-sm);
  }
  .thumb {
    position: relative;
    width: 72px;
    height: 54px;
    border-radius: 10px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--surface-2);
    color: var(--text-3);
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: fadein var(--t-slow) var(--ease-out);
  }
  @keyframes fadein {
    from {
      opacity: 0;
    }
  }
  .tick {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(31, 138, 60, 0.82);
    color: #fff;
    animation: pop var(--t-slow) var(--spring);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(1.3);
    }
  }
  .shimmer {
    position: absolute;
    inset: 0;
    background: linear-gradient(100deg, transparent 20%, var(--fill) 50%, transparent 80%);
    background-size: 200% 100%;
    animation: shimmer 1.2s linear infinite;
  }
  @keyframes shimmer {
    from {
      background-position: 120% 0;
    }
    to {
      background-position: -80% 0;
    }
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .name {
    font-weight: 620;
    font-size: 15px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .status {
    font-size: 13px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row {
    transition: border-color var(--t-med) var(--ease-out), box-shadow var(--t-med) var(--ease-out),
      background-color var(--t-med);
  }
  .row.running {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--separator));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent), var(--shadow-sm);
  }
  .row.good {
    animation: done-glow 1.1s var(--ease-out);
  }
  @keyframes done-glow {
    0% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--success) 55%, transparent);
    }
    100% {
      box-shadow: 0 0 0 14px transparent;
    }
  }
  .queued .status {
    animation: breathe 1.8s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      opacity: 0.45;
    }
  }
  .busy {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.42);
    color: #fff;
    animation: fadein var(--t-med) var(--ease-out);
  }
  .ring {
    width: 40px;
    height: 40px;
    transform: rotate(-90deg);
    grid-area: 1 / 1;
  }
  .ring circle {
    fill: none;
    stroke-width: 3.4;
  }
  .ring .track {
    stroke: rgba(255, 255, 255, 0.25);
  }
  .ring .arc {
    stroke: #fff;
    stroke-linecap: round;
  }
  .glyph {
    grid-area: 1 / 1;
    display: grid;
    place-items: center;
  }
  .download .glyph {
    animation: drop 1.1s var(--ease-in-out, ease-in-out) infinite;
  }
  @keyframes drop {
    0% {
      transform: translateY(-5px);
      opacity: 0;
    }
    35%,
    70% {
      transform: translateY(0);
      opacity: 1;
    }
    100% {
      transform: translateY(5px);
      opacity: 0;
    }
  }
  .convert .glyph {
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      transform: scale(0.82);
    }
  }
  .status {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .stage-text {
    animation: rise var(--t-slow) var(--ease-out);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(5px);
    }
  }
  .step {
    flex: none;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.02em;
    padding: 2px 7px;
    border-radius: 999px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    transition: color var(--t-med), background-color var(--t-med);
  }
  .step.convert {
    color: var(--success);
    background: color-mix(in srgb, var(--success) 14%, transparent);
  }
  .meta {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    color: var(--text-2);
  }
  .meta .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta .left {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 600;
    color: var(--text);
  }
  @media (prefers-reduced-motion: reduce) {
    .download .glyph,
    .convert .glyph,
    .queued .status,
    .row.good {
      animation: none;
    }
  }
  .good .status {
    color: var(--success);
    font-weight: 600;
  }
  .bad .status {
    color: var(--danger);
    white-space: normal;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 2px;
    color: var(--text-2);
  }
  .tools .icon {
    border-radius: 50%;
  }
  .show {
    --h: 34px;
    padding: 0 10px;
    color: var(--accent);
    font-size: 13.5px;
  }
  @media (max-width: 760px) {
    .thumb {
      width: 60px;
      height: 45px;
    }
    .row {
      gap: 11px;
    }
  }
</style>
