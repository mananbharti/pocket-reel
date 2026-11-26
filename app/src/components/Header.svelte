<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { remaining } from "../lib/format";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";

  let { onprefs }: { onprefs: () => void } = $props();

  const running = $derived(app.active.filter((j) => j.state === "running"));
  const status = $derived.by(() => {
    const total = app.active.length;
    if (!total) return null;
    // Remaining time across the whole queue: running ETAs plus queued durations at the observed speed.
    const speed = running.length ? running.reduce((s, j) => s + (j.progress?.speed ?? 0), 0) / running.length : 0;
    let secs = 0;
    let known = true;
    for (const j of app.active) {
      if (j.state === "running") {
        if (j.progress?.etaSeconds == null) known = false;
        else secs += j.progress.etaSeconds;
      } else if (speed > 0) secs += j.duration / speed;
      else known = false;
    }
    const parallel = Math.max(1, Math.min(app.prefs.concurrency, total));
    return {
      text: `Converting ${total} file${total === 1 ? "" : "s"}`,
      eta: known ? remaining(secs / parallel) : "Estimating time…",
    };
  });
</script>

<header class="top">
  <div class="brand">
    <Logo size={30} />
    <h1>Pocket Reel</h1>
  </div>
  <div class="status" aria-hidden="true">
    {#if status}
      <span class="pulse"></span>
      <span class="s1">{status.text}</span>
      <span class="s2 num">{status.eta}</span>
    {/if}
  </div>
  <div class="actions">
    <button type="button" class="btn add" onclick={() => app.pickFiles()} aria-keyshortcuts="Control+O">
      <Icon name="plus" size={18} />
      <span>Add Videos</span>
    </button>
    <button type="button" class="btn icon plain" aria-label="Preferences" onclick={onprefs}>
      <Icon name="sliders" size={20} />
    </button>
  </div>
</header>

<style>
  .top {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 12px;
    height: 56px;
    padding: 0 16px 0 18px;
    padding-top: env(safe-area-inset-top);
    box-sizing: content-box;
    background: var(--material);
    backdrop-filter: saturate(180%) blur(20px);
    -webkit-backdrop-filter: saturate(180%) blur(20px);
    border-bottom: 1px solid var(--separator);
    position: relative;
    z-index: 5;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 680;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .s1 {
    color: var(--text);
    font-weight: 560;
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
  .actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 6px;
  }
  .add {
    color: var(--accent);
    background: var(--accent-soft);
    border-color: transparent;
  }
  .add:hover:not(:disabled) {
    background: var(--accent-soft);
    filter: brightness(0.97);
  }
  @media (max-width: 760px) {
    .top {
      grid-template-columns: 1fr auto;
      padding: 0 8px 0 14px;
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
