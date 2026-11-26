<script lang="ts">
  import { app, type Item } from "../lib/store.svelte";
  import { bytes, bytesSpoken, clock, durationWords, remaining } from "../lib/format";
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

  const status = $derived.by(() => {
    if (item.status === "probing") return item.isLink ? "Getting video details…" : "Reading…";
    if (item.status === "error") return item.error ?? "Can't read this file";
    if (!job) {
      if (item.isLink) return `From ${site}${length ? ` · ${clock(length)}` : ""}`;
      return item.estimate && !item.estimating ? `${clock(length)} · about ${bytes(item.estimate.bytes)}` : clock(length);
    }
    if (job.state === "queued") return "Waiting…";
    if (downloading) return `Downloading ${pct}% · ${remaining(job.progress?.etaSeconds)}`;
    if (job.state === "running") return `Converting ${pct}% · ${remaining(job.progress?.etaSeconds)}`;
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

<div class="row {tone}" role="group" aria-label={spoken}>
  <span class="thumb" aria-hidden="true">
    {#if item.thumb}
      <img src={item.thumb} alt="" />
    {:else if item.status === "probing"}
      <span class="shimmer"></span>
    {:else}
      <Icon name={item.isLink ? "globe" : "film"} size={22} stroke={1.6} />
    {/if}
    {#if job?.state === "done"}
      <span class="tick"><Icon name="check" size={16} stroke={3} /></span>
    {/if}
  </span>

  <span class="info">
    <span class="name" title={item.isLink ? item.path : undefined}>{item.name}</span>
    <span class="status num">{status}</span>
    {#if busy}
      <ProgressBar
        value={job?.state === "queued" ? 0 : (job?.progress?.fraction ?? 0)}
        label="Converting {item.name}"
        valueText={job?.state === "queued" ? "Waiting" : `${downloading ? "Downloading" : "Converting"} ${pct}%, ${remaining(job?.progress?.etaSeconds)}`}
      />
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
