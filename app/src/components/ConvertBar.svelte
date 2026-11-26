<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { bytes, timeLeft } from "../lib/format";
  import { CLASSIC_SCREENS, QUALITIES } from "../lib/screens";
  import Icon from "./Icon.svelte";
  import ProgressBar from "./ProgressBar.svelte";

  // Phones: short option names, with the recommendation in the field label.
  let narrow = $state(false);
  $effect(() => {
    const mq = window.matchMedia("(max-width: 560px)");
    narrow = mq.matches;
    const on = () => (narrow = mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  });

  const batch = $derived(app.batchProgress);
  const batchPct = $derived(Math.floor((batch?.fraction ?? 0) * 100));
  const batchWhat = $derived.by(() => {
    if (!batch) return "";
    const parts: string[] = [];
    if (batch.downloading) parts.push(`Downloading ${batch.downloading}`);
    if (batch.converting) parts.push(`Converting ${batch.converting}`);
    if (batch.waiting) parts.push(`${batch.waiting} waiting`);
    return parts.join(" · ") || "Finishing…";
  });

  const count = $derived(app.pending.length || app.convertible.length);
  const again = $derived(!app.pending.length && app.convertible.length > 0);
  const busy = $derived(app.active.length > 0 && app.pending.length === 0);
  const label = $derived(
    busy
      ? `Working… ${batchPct}%`
      : count === 0
        ? "Convert"
        : again
          ? "Convert again"
          : `Convert ${count} ${count === 1 ? "video" : "videos"}`,
  );

  async function chooseFolder() {
    const dir = await app.api?.pickFolder();
    if (dir) app.setPref("outputDir", dir);
  }
  const folderName = $derived(app.prefs.outputDir?.split(/[\\/]/).filter(Boolean).pop() ?? "");
</script>

<div class="bar">
  {#if batch?.active}
    <div class="overall" transition:slide={{ duration: 320, easing: cubicOut }} aria-live="polite">
      <div class="ov-top num">
        <span class="ov-what">{batchWhat}</span>
        <span class="ov-count">{batch.finished} of {batch.total} done</span>
      </div>
      <ProgressBar
        value={batch.fraction}
        size="large"
        stage={batch.downloading && !batch.converting ? "download" : "convert"}
        label="All videos"
        valueText="{batchPct}%, {timeLeft(batch.eta)}"
      />
      <div class="ov-bottom num">
        <span>{batchPct}%</span>
        <span class="ov-eta"><Icon name="clock" size={14} stroke={2} />{batch.eta === 0 ? "Done" : timeLeft(batch.eta)}</span>
      </div>
    </div>
  {/if}
  <div class="opts">
    <label class="opt">
      <span class="lbl">Player screen</span>
      <select value={app.prefs.screen} onchange={(e) => app.setScreen(e.currentTarget.value)}>
        {#each CLASSIC_SCREENS as s (s)}
          <option value={s}>{s.replace("x", " × ")}{s === "160x120" ? " (most players)" : ""}</option>
        {/each}
      </select>
    </label>
    {#if app.canDownload}
      <label class="opt">
        <span class="lbl">Link quality{narrow ? " · 360p best" : ""}</span>
        <select value={app.prefs.quality} onchange={(e) => app.setPref("quality", Number(e.currentTarget.value))}>
          {#each QUALITIES as q (q.value)}
            <option value={q.value}>{narrow ? q.short : q.label}</option>
          {/each}
        </select>
      </label>
    {/if}
    {#if app.api?.supportsFolders}
      <div class="opt">
        <span class="lbl" id="save-lbl">Save to</span>
        <div class="save">
          <button type="button" class="pick" aria-describedby="save-lbl" onclick={chooseFolder} title={app.prefs.outputDir ?? ""}>
            <Icon name="folder" size={17} />
            <span>{folderName || "Next to each video"}</span>
          </button>
          {#if app.prefs.outputDir}
            <button type="button" class="reset" aria-label="Save next to each video instead" onclick={() => app.setPref("outputDir", null)}>
              <Icon name="x" size={14} />
            </button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
  <div class="go-wrap">
    {#if app.pending.length && !app.estimatesPending}
      <span class="total num">about {bytes(app.totalEstimate)}</span>
    {/if}
    {#if app.active.length}
      <button type="button" class="btn large" onclick={() => app.cancelAll()}>
        <Icon name="stop" size={15} /> Stop
      </button>
    {/if}
    <button
      type="button"
      class="btn primary large go"
      disabled={busy || count === 0}
      aria-keyshortcuts="Control+Enter"
      onclick={() => app.convert()}
    >
      {label}
    </button>
  </div>
</div>

<style>
  .bar {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 14px 20px;
    padding: 14px max(20px, calc((100vw - 760px) / 2));
    padding-bottom: calc(14px + env(safe-area-inset-bottom) + var(--nav-bottom, 0px));
    background: var(--material);
    backdrop-filter: saturate(180%) blur(20px);
    -webkit-backdrop-filter: saturate(180%) blur(20px);
    border-top: 1px solid var(--separator);
  }
  .overall {
    flex: 1 0 100%;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 12px 14px;
    border-radius: var(--r-lg);
    background: var(--bg-elevated);
    border: 1px solid var(--separator);
    box-shadow: var(--shadow-sm);
  }
  .ov-top,
  .ov-bottom {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .ov-what {
    font-weight: 650;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ov-count,
  .ov-bottom {
    color: var(--text-2);
  }
  .ov-eta {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-weight: 650;
    color: var(--text);
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 18px;
    min-width: 0;
  }
  .opt {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .lbl {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  select,
  .pick {
    height: 40px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--separator-strong);
    background: var(--bg-elevated);
    font-size: 14.5px;
    font-weight: 560;
  }
  select {
    padding-right: 30px;
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%2386868b' stroke-width='2.2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M6.5 9.5 12 15l5.5-5.5'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 9px center;
    background-size: 16px;
  }
  .save {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .pick {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    max-width: 240px;
    color: var(--text);
  }
  .pick span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pick :global(svg) {
    color: var(--accent);
    flex: none;
  }
  .reset {
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text-3);
    display: grid;
    place-items: center;
  }
  .reset:hover {
    background: var(--fill);
  }
  .go-wrap {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-left: auto;
  }
  .total {
    font-size: 13px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .go {
    min-width: 190px;
  }
  @media (max-width: 560px) {
    .opts {
      width: 100%;
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    }
    .opt,
    select {
      width: 100%;
    }
    select {
      font-size: 14px;
      padding: 0 26px 0 10px;
      background-position: right 7px center;
    }
    .go-wrap {
      width: 100%;
    }
    .go {
      flex: 1;
      min-width: 0;
    }
  }
</style>
