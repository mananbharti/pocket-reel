<script lang="ts">
  import { app, type Item } from "../lib/store.svelte";
  import { bytes, bytesSpoken, clock, codecName, durationWords, fps, remaining } from "../lib/format";
  import Icon from "./Icon.svelte";
  import ProgressBar from "./ProgressBar.svelte";

  interface Props {
    item: Item;
    onnavigate: (dir: 1 | -1) => void;
  }
  let { item, onnavigate }: Props = $props();

  const job = $derived(app.jobFor(item));
  const selected = $derived(app.selectedKey === item.key);
  const m = $derived(item.media);
  const busy = $derived(job?.state === "running" || job?.state === "queued");
  const trimmed = $derived(item.trimStart != null || item.trimEnd != null);

  const meta = $derived.by(() => {
    if (!m) return "";
    const parts = [clock(m.duration)];
    if (m.video) parts.push(`${m.video.displayWidth}×${m.video.displayHeight}`, `${fps(m.video.fps)} fps`);
    else if (m.audio) parts.push(codecName(m.audio.codec), `${m.audio.channels > 1 ? "Stereo" : "Mono"}`);
    parts.push(bytes(m.sizeBytes));
    return parts.join(" · ");
  });

  const spoken = $derived.by(() => {
    const p = [item.name];
    if (m) p.push(durationWords(m.duration));
    if (item.status === "error") p.push(`cannot be read: ${item.error}`);
    else if (job?.state === "running") p.push(`converting, ${Math.round((job.progress?.fraction ?? 0) * 100)} percent`);
    else if (job?.state === "queued") p.push("waiting in queue");
    else if (job?.state === "done") p.push(`converted, ${bytesSpoken(job.outputBytes)}`);
    else if (job?.state === "failed") p.push(`failed: ${job.error}`);
    else if (item.estimate) p.push(`estimated output ${bytesSpoken(item.estimate.bytes)}`);
    if (trimmed) p.push("trimmed");
    return p.join(", ");
  });

  function key(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      onnavigate(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      onnavigate(-1);
    } else if ((e.key === "Delete" || e.key === "Backspace") && !busy) {
      e.preventDefault();
      app.removeItem(item.key);
    }
  }
</script>

<div class="row" class:selected class:done={job?.state === "done"} class:failed={job?.state === "failed" || item.status === "error"}>
  <button
    type="button"
    class="select"
    data-key={item.key}
    aria-pressed={selected}
    aria-label={spoken}
    onclick={() => app.select(item.key)}
    onkeydown={key}
  >
    <span class="thumb" aria-hidden="true">
      {#if item.thumb}
        <img src={item.thumb} alt="" />
      {:else if item.status === "probing"}
        <span class="shimmer"></span>
      {:else}
        <Icon name={m && !m.video ? "music" : "film"} size={22} stroke={1.6} />
      {/if}
    </span>
    <span class="info" aria-hidden="true">
      <span class="name">{item.name}</span>
      <span class="meta num">
        {#if item.status === "probing"}
          Reading…
        {:else if item.status === "error"}
          <span class="err">{item.error}</span>
        {:else}
          {meta}{#if trimmed}<span class="tag"><Icon name="scissors" size={12} stroke={2} /> Trimmed</span>{/if}
        {/if}
      </span>
    </span>
    <span class="side" aria-hidden="true">
      {#if job?.state === "done"}
        <span class="badge ok"><Icon name="check" size={14} stroke={2.4} /> {bytes(job.outputBytes)}</span>
      {:else if job?.state === "running" || job?.state === "queued"}
        <span class="pct num">{job.state === "queued" ? "Waiting" : `${Math.floor((job.progress?.fraction ?? 0) * 100)}%`}</span>
      {:else if item.planError}
        <span class="warn">Not supported</span>
      {:else if item.estimate && !item.estimating}
        <span class="est num">≈ {bytes(item.estimate.bytes)}</span>
      {:else if item.status === "ready"}
        <span class="est muted">Estimating…</span>
      {/if}
    </span>
  </button>

  <div class="tools">
    {#if busy && job}
      <button type="button" class="btn icon plain" aria-label="Cancel converting {item.name}" onclick={() => app.cancel(job.id)}>
        <Icon name="stop" size={18} />
      </button>
    {:else if job?.state === "done" && !app.isMobile}
      <button type="button" class="btn icon plain" aria-label="Show {item.name} in folder" title="Show in folder" onclick={() => app.api?.reveal(job.output)}>
        <Icon name="reveal" size={18} />
      </button>
      <button type="button" class="btn icon plain" aria-label="Open converted {item.name}" title="Open" onclick={() => app.api?.open(job.output)}>
        <Icon name="play" size={16} />
      </button>
    {:else if job?.state === "failed" || job?.state === "canceled"}
      <button type="button" class="btn icon plain" aria-label="Try {item.name} again" onclick={() => app.retry(item)}>
        <Icon name="retry" size={18} />
      </button>
    {/if}
    {#if !busy}
      <button type="button" class="btn icon plain remove" aria-label="Remove {item.name}" onclick={() => app.removeItem(item.key)}>
        <Icon name="x" size={17} />
      </button>
    {/if}
  </div>

  {#if job && job.state !== "done"}
    <div class="progress">
      {#if job.state === "failed"}
        <p class="msg err">{job.error}</p>
      {:else if job.state === "canceled"}
        <p class="msg muted">Canceled</p>
      {:else}
        <ProgressBar
          value={job.state === "queued" ? 0 : (job.progress?.fraction ?? 0)}
          label="Converting {item.name}"
          valueText={job.state === "queued" ? "Waiting" : `${Math.floor((job.progress?.fraction ?? 0) * 100)}%, ${remaining(job.progress?.etaSeconds)}`}
        />
        <p class="msg num" aria-hidden="true">
          {#if job.state === "queued"}
            Waiting for other conversions
          {:else}
            {remaining(job.progress?.etaSeconds)}{job.progress && job.progress.speed > 0 ? ` · ${job.progress.speed.toFixed(1)}× speed` : ""}
          {/if}
        </p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .row {
    position: relative;
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: var(--border-width) solid var(--separator);
    box-shadow: var(--shadow-sm);
    transition: box-shadow var(--t-med) var(--ease-out), background-color var(--t-med);
  }
  .row.selected {
    box-shadow: 0 0 0 2px var(--accent), var(--shadow-md);
  }
  .select {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 14px;
    min-width: 0;
    padding: 12px 6px 12px 12px;
    border: 0;
    background: transparent;
    text-align: left;
    border-radius: var(--r-lg);
  }
  .thumb {
    width: 76px;
    height: 50px;
    border-radius: 9px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--surface-2);
    color: var(--text-3);
    position: relative;
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
      transform: scale(1.04);
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
    gap: 3px;
    min-width: 0;
  }
  .name {
    font-weight: 600;
    font-size: 14.5px;
    letter-spacing: -0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    font-size: 12.5px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 7px 1px 5px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
    font-size: 11px;
  }
  .side {
    font-size: 13px;
    white-space: nowrap;
  }
  .est {
    font-weight: 600;
    color: var(--text);
  }
  .muted,
  .est.muted {
    color: var(--text-3);
    font-weight: 500;
  }
  .pct {
    font-weight: 650;
    color: var(--accent);
  }
  .warn {
    color: var(--warning);
    font-weight: 560;
  }
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 9px 3px 7px;
    border-radius: 999px;
    font-weight: 620;
    font-size: 12.5px;
  }
  .badge.ok {
    background: var(--success-soft);
    color: var(--success);
    animation: pop var(--t-slow) var(--spring);
  }
  @keyframes pop {
    from {
      transform: scale(0.6);
      opacity: 0;
    }
  }
  .err {
    color: var(--danger);
  }
  .tools {
    display: flex;
    gap: 2px;
    padding-right: 8px;
    color: var(--text-2);
  }
  .tools .btn {
    border-radius: 50%;
  }
  .remove {
    opacity: 0.55;
  }
  .row:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }
  .progress {
    grid-column: 1 / -1;
    padding: 0 16px 12px 102px;
    animation: expand var(--t-slow) var(--spring-soft);
  }
  @keyframes expand {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .msg {
    margin: 6px 0 0;
    font-size: 12px;
    color: var(--text-2);
  }
  .msg.err {
    margin: 0;
    color: var(--danger);
  }
  @media (max-width: 760px) {
    .thumb {
      width: 60px;
      height: 42px;
    }
    .select {
      gap: 11px;
    }
    .progress {
      padding-left: 12px;
    }
    .remove {
      opacity: 1;
    }
  }
</style>
