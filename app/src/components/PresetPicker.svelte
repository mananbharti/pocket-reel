<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { CATEGORY_TITLES, presetIcon } from "../lib/presetIcon";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  const groups = $derived(
    (["classic", "modern", "audio"] as const).map((c) => ({
      id: c,
      title: CATEGORY_TITLES[c],
      presets: app.presets.filter((p) => p.category === c),
    })),
  );
  const flat = $derived(groups.flatMap((g) => g.presets));
  let buttons: Record<string, HTMLButtonElement> = $state({});

  $effect(() => {
    if (open) setTimeout(() => buttons[app.settings.preset]?.focus(), 60);
  });

  function choose(id: string) {
    app.setPreset(id);
    onclose();
  }

  function key(e: KeyboardEvent, id: string) {
    const i = flat.findIndex((p) => p.id === id);
    let j = i;
    if (e.key === "ArrowDown" || e.key === "ArrowRight") j = Math.min(flat.length - 1, i + 1);
    else if (e.key === "ArrowUp" || e.key === "ArrowLeft") j = Math.max(0, i - 1);
    else if (e.key === "Home") j = 0;
    else if (e.key === "End") j = flat.length - 1;
    else return;
    e.preventDefault();
    buttons[flat[j].id]?.focus();
  }
</script>

<Dialog {open} {onclose} title="Convert to" width={520}>
  <div role="radiogroup" aria-label="Output format">
    {#each groups as g (g.id)}
      <h3 class="grp-title" id="grp-{g.id}">{g.title}</h3>
      <div class="list">
        {#each g.presets as p (p.id)}
          <button
            bind:this={buttons[p.id]}
            type="button"
            role="radio"
            class="opt"
            aria-checked={p.id === app.settings.preset}
            aria-describedby="desc-{p.id}"
            disabled={!p.available}
            tabindex={p.id === app.settings.preset ? 0 : -1}
            onclick={() => choose(p.id)}
            onkeydown={(e) => key(e, p.id)}
          >
            <span class="ico"><Icon name={presetIcon(p)} size={22} stroke={1.6} /></span>
            <span class="txt">
              <span class="name">
                {p.name}
                {#if p.original}<span class="badge">Original tool</span>{/if}
              </span>
              <span class="fmt">{p.formatLabel}</span>
              <span class="desc" id="desc-{p.id}">{p.available ? p.description : p.unavailableReason}</span>
            </span>
            <span class="tick" aria-hidden="true"><Icon name="check" size={18} stroke={2.4} /></span>
          </button>
        {/each}
      </div>
    {/each}
  </div>
</Dialog>

<style>
  .grp-title {
    margin: 14px 2px 8px;
    font-size: 12px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .grp-title:first-child {
    margin-top: 4px;
  }
  .list {
    display: flex;
    flex-direction: column;
    border-radius: var(--r-lg);
    background: var(--surface-2);
    border: var(--border-width) solid var(--separator);
    overflow: hidden;
  }
  .opt {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border: 0;
    background: transparent;
    text-align: left;
    transition: background-color var(--t-fast) var(--ease-out);
  }
  .opt + .opt {
    border-top: 1px solid var(--separator);
  }
  .opt:hover:not(:disabled) {
    background: var(--fill);
  }
  .opt:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .opt:focus-visible {
    outline-offset: -3px;
    border-radius: var(--r-md);
  }
  .ico {
    width: 40px;
    height: 40px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    background: var(--bg-elevated);
    color: var(--accent);
    box-shadow: var(--shadow-sm);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .name {
    font-weight: 620;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .badge {
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .fmt {
    font-size: 12.5px;
    color: var(--text-2);
    font-weight: 520;
  }
  .desc {
    font-size: 12.5px;
    color: var(--text-3);
    line-height: 1.35;
    margin-top: 2px;
  }
  .tick {
    color: var(--accent);
    opacity: 0;
    transform: scale(0.5);
    transition: opacity var(--t-fast), transform var(--t-med) var(--spring);
  }
  .opt[aria-checked="true"] .tick {
    opacity: 1;
    transform: none;
  }
  .opt[aria-checked="true"] .ico {
    background: var(--accent);
    color: var(--accent-text);
  }
</style>
