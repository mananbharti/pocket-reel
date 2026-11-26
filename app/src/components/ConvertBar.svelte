<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { bytes } from "../lib/format";
  import Icon from "./Icon.svelte";

  const count = $derived(app.pending.length || app.convertible.length);
  const again = $derived(!app.pending.length && app.convertible.length > 0);
  const busyOnly = $derived(app.active.length > 0 && app.pending.length === 0);
  const label = $derived(
    busyOnly
      ? "Converting…"
      : count === 0
        ? "Convert"
        : again
          ? `Convert ${count === 1 ? "Again" : `${count} Again`}`
          : `Convert ${count} ${count === 1 ? "File" : "Files"}`,
  );
</script>

<div class="bar">
  <div class="summary num" aria-live="off">
    {#if app.items.length === 0}
      <span class="muted">Add videos to begin</span>
    {:else if app.pending.length}
      <span class="big">{app.estimatesPending ? "Estimating…" : `≈ ${bytes(app.totalEstimate)}`}</span>
      <span class="muted">total output</span>
    {:else if app.active.length}
      <span class="visually-hidden">{app.active.length} in queue</span>
    {:else}
      <span class="muted">All done</span>
    {/if}
  </div>
  {#if app.active.length}
    <button type="button" class="btn large" onclick={() => app.cancelAll()}>
      <Icon name="stop" size={16} /> Cancel All
    </button>
  {/if}
  <button
    type="button"
    class="btn primary large go"
    disabled={busyOnly || count === 0 || !app.preset?.available}
    aria-keyshortcuts="Control+Enter"
    onclick={() => app.convert()}
  >
    <Icon name="arrowDown" size={19} stroke={2} />
    {label}
  </button>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    padding-bottom: calc(12px + env(safe-area-inset-bottom));
    background: var(--material);
    backdrop-filter: saturate(180%) blur(20px);
    -webkit-backdrop-filter: saturate(180%) blur(20px);
    border-top: 1px solid var(--separator);
  }
  .summary {
    flex: 1;
    display: flex;
    flex-direction: column;
    line-height: 1.2;
    min-width: 0;
  }
  .big {
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .muted {
    font-size: 12.5px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .go {
    min-width: 168px;
  }
  @media (max-width: 420px) {
    .go {
      min-width: 0;
      flex: 1;
    }
  }
</style>
