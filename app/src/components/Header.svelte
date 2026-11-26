<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { timeLeft } from "../lib/format";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";

  // Same estimate as the overall progress panel.
  const status = $derived.by(() => {
    const b = app.batchProgress;
    if (!b?.active) return app.active.length ? "Working…" : null;
    const doing = b.downloading && !b.converting ? "Downloading" : "Converting";
    return `${doing} · ${Math.floor(b.fraction * 100)}% · ${timeLeft(b.eta)}`;
  });
</script>

<header class="top">
  <div class="inner">
    <div class="brand">
      <Logo size={32} />
      <div class="names">
        <h1>Pocket Reel</h1>
        <span class="sub">Videos for your MP3 player</span>
      </div>
    </div>
    {#if status}
      <span class="status num" aria-hidden="true"><span class="pulse"></span>{status}</span>
    {/if}
    {#if app.ready && app.items.length > 0}
      <button type="button" class="btn add" onclick={() => app.pickFiles()} aria-keyshortcuts="Control+O">
        <Icon name="plus" size={18} />
        <span>Add videos</span>
      </button>
    {/if}
  </div>
</header>

<style>
  .top {
    padding-top: calc(env(safe-area-inset-top) + var(--status-top, 0px));
    background: var(--material);
    backdrop-filter: saturate(180%) blur(20px);
    -webkit-backdrop-filter: saturate(180%) blur(20px);
    border-bottom: 1px solid var(--separator);
  }
  .inner {
    max-width: 760px;
    margin: 0 auto;
    height: 64px;
    padding: 0 20px;
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    min-width: 0;
    margin-right: auto;
  }
  .names {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .sub {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .pulse {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1.6s var(--ease-in-out) infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      transform: scale(0.75);
      opacity: 0.55;
    }
    50% {
      transform: scale(1);
      opacity: 1;
    }
  }
  .add {
    color: var(--accent);
    background: var(--accent-soft);
    border-color: transparent;
  }
  @media (max-width: 560px) {
    .inner {
      padding: 0 14px;
    }
    .status {
      display: none;
    }
    .add span {
      display: none;
    }
    .add {
      width: var(--h);
      padding: 0;
    }
  }
</style>
