<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  // Native <dialog> (focus trapping, Escape, inert background) with spring
  // open/close transitions. Becomes a bottom sheet on narrow screens.
  interface Props {
    open: boolean;
    title: string;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
    width?: number;
  }
  let { open, title, onclose, children, footer, width = 460 }: Props = $props();

  let el: HTMLDialogElement | undefined = $state();
  let shown = $state(false);
  const titleId = `dlg-${Math.random().toString(36).slice(2, 8)}`;
  let closeTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    const d = el;
    if (!d) return;
    if (open && !d.open) {
      if (closeTimer) clearTimeout(closeTimer);
      d.showModal();
      requestAnimationFrame(() => requestAnimationFrame(() => (shown = true)));
    } else if (!open && d.open) {
      shown = false;
      closeTimer = setTimeout(() => d.close(), 240);
    }
  });

  function backdrop(e: MouseEvent) {
    if (e.target === el) onclose();
  }
</script>

<dialog
  bind:this={el}
  class:shown
  aria-labelledby={titleId}
  style="--w: {width}px"
  oncancel={(e) => {
    e.preventDefault();
    onclose();
  }}
  onclick={backdrop}
>
  <div class="panel">
    <header>
      <h2 id={titleId}>{title}</h2>
      <button type="button" class="btn icon plain close" aria-label="Close" onclick={onclose}>
        <Icon name="x" size={18} />
      </button>
    </header>
    <div class="body">
      {@render children()}
    </div>
    {#if footer}
      <footer>{@render footer()}</footer>
    {/if}
  </div>
</dialog>

<style>
  dialog {
    padding: 0;
    border: 0;
    background: transparent;
    max-width: none;
    max-height: none;
    width: 100%;
    height: 100%;
    margin: 0;
    display: none;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  dialog[open] {
    display: flex;
  }
  dialog::backdrop {
    background: var(--backdrop);
    opacity: 0;
    transition: opacity var(--t-med) var(--ease-out);
  }
  dialog.shown::backdrop {
    opacity: 1;
  }
  .panel {
    width: min(var(--w), calc(100vw - 32px));
    max-height: min(720px, calc(100vh - 48px));
    display: flex;
    flex-direction: column;
    background: var(--bg-elevated);
    border-radius: var(--r-xl);
    border: var(--border-width) solid var(--separator);
    box-shadow: var(--shadow-lg);
    opacity: 0;
    transform: scale(0.94) translateY(8px);
    transition: transform var(--t-slow) var(--spring), opacity var(--t-med) var(--ease-out);
  }
  .shown .panel {
    opacity: 1;
    transform: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 14px 8px 22px;
  }
  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 19px;
    font-weight: 680;
    letter-spacing: -0.02em;
  }
  .close {
    border-radius: 50%;
    color: var(--text-2);
  }
  .body {
    overflow-y: auto;
    padding: 6px 22px 22px;
    overscroll-behavior: contain;
  }
  footer {
    padding: 12px 22px 18px;
    border-top: 1px solid var(--separator);
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  @media (max-width: 640px) {
    dialog[open] {
      align-items: flex-end;
    }
    .panel {
      width: 100%;
      max-height: 88vh;
      border-radius: var(--r-xl) var(--r-xl) 0 0;
      transform: translateY(100%);
      padding-bottom: env(safe-area-inset-bottom);
    }
    .shown .panel {
      transform: none;
    }
  }
</style>
