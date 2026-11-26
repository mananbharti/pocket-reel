<script lang="ts">
  import { flip } from "svelte/animate";
  import { app } from "../lib/store.svelte";
  import { fadeScale, springIn } from "../lib/motion";
  import Icon from "./Icon.svelte";
</script>

<!-- Visual only: the same messages are announced through the live region. -->
<div class="toasts" aria-hidden="true">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}" animate:flip={{ duration: 300 }} in:springIn={{ y: 24 }} out:fadeScale>
      <Icon name={t.kind === "success" ? "check" : t.kind === "error" ? "warning" : "info"} size={18} stroke={2} />
      <span>{t.text}</span>
      <button type="button" tabindex="-1" class="x" onclick={() => app.dismissToast(t.id)}><Icon name="x" size={14} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    bottom: calc(92px + env(safe-area-inset-bottom));
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    z-index: 50;
    pointer-events: none;
    width: min(520px, calc(100vw - 24px));
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 100%;
    padding: 10px 10px 10px 14px;
    border-radius: 14px;
    background: var(--bg-elevated);
    border: var(--border-width) solid var(--separator);
    box-shadow: var(--shadow-lg);
    font-size: 13.5px;
    font-weight: 560;
  }
  .toast span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .success {
    color: var(--text);
  }
  .success :global(svg) {
    color: var(--success);
  }
  .error :global(svg:first-child) {
    color: var(--danger);
  }
  .x {
    border: 0;
    background: transparent;
    color: var(--text-3);
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    padding: 0;
  }
</style>
