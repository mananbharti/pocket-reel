<script lang="ts">
  import { app, type Item } from "../lib/store.svelte";
  import { clock, clockPrecise, durationWords, parseClock } from "../lib/format";
  import Icon from "./Icon.svelte";

  let { item }: { item: Item } = $props();
  const dur = $derived(item.media?.duration ?? 0);
  const start = $derived(item.trimStart ?? 0);
  const end = $derived(item.trimEnd ?? dur);
  const STEP = 0.1;

  function setStart(v: number) {
    const s = Math.min(Math.max(0, v), end - 0.5);
    app.setTrim(item.key, s > 0.05 ? s : null, item.trimEnd);
  }
  function setEnd(v: number) {
    const e = Math.max(Math.min(dur, v), start + 0.5);
    app.setTrim(item.key, item.trimStart, e < dur - 0.05 ? e : null);
  }
  function text(which: "start" | "end", t: string) {
    const v = parseClock(t);
    if (v == null) return;
    if (which === "start") setStart(v);
    else setEnd(v);
  }
  const active = $derived(item.trimStart != null || item.trimEnd != null);
</script>

<div class="trim">
  <div class="track" aria-hidden="true">
    <span class="sel" style="left: {(start / dur) * 100}%; right: {100 - (end / dur) * 100}%"></span>
  </div>
  <div class="row">
    <label class="lbl" for="trim-start">Start</label>
    <input
      id="trim-start"
      class="slider"
      type="range"
      min="0"
      max={dur}
      step={STEP}
      value={start}
      aria-valuetext={durationWords(start)}
      style="--fill: {(start / dur) * 100}%"
      oninput={(e) => setStart(Number(e.currentTarget.value))}
    />
    <input
      class="time num"
      type="text"
      aria-label="Start time"
      value={clockPrecise(start)}
      onchange={(e) => text("start", e.currentTarget.value)}
    />
  </div>
  <div class="row">
    <label class="lbl" for="trim-end">End</label>
    <input
      id="trim-end"
      class="slider"
      type="range"
      min="0"
      max={dur}
      step={STEP}
      value={end}
      aria-valuetext={durationWords(end)}
      style="--fill: {(end / dur) * 100}%"
      oninput={(e) => setEnd(Number(e.currentTarget.value))}
    />
    <input
      class="time num"
      type="text"
      aria-label="End time"
      value={clockPrecise(end)}
      onchange={(e) => text("end", e.currentTarget.value)}
    />
  </div>
  <div class="foot">
    <span class="num">Output length <strong>{clock(end - start)}</strong> of {clock(dur)}</span>
    {#if active}
      <button type="button" class="btn plain reset" onclick={() => app.setTrim(item.key, null, null)}>
        <Icon name="retry" size={15} /> Reset
      </button>
    {/if}
  </div>
</div>

<style>
  .trim {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .track {
    position: relative;
    height: 8px;
    border-radius: 999px;
    background: var(--fill);
    margin: 4px 0 8px;
    overflow: hidden;
  }
  .sel {
    position: absolute;
    top: 0;
    bottom: 0;
    background: var(--accent);
    opacity: 0.85;
    border-radius: 999px;
    transition: left 180ms var(--ease-out), right 180ms var(--ease-out);
  }
  .row {
    display: grid;
    grid-template-columns: 40px 1fr 84px;
    align-items: center;
    gap: 10px;
  }
  .lbl {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
  }
  .time {
    width: 84px;
    text-align: center;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 6px;
    font-size: 12.5px;
    color: var(--text-2);
    min-height: 30px;
  }
  .foot strong {
    color: var(--text);
  }
  .reset {
    --h: 28px;
    font-size: 13px;
    color: var(--accent);
    padding: 0 8px;
  }
</style>
