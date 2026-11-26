<script lang="ts" generics="T extends string | number">
  // Wrapping chips that behave as one ARIA radio group.
  interface Option {
    value: T;
    label: string;
    sub?: string;
  }
  interface Props {
    options: Option[];
    value: T | null;
    label: string;
    onchange: (v: T) => void;
  }
  let { options, value, label, onchange }: Props = $props();
  let buttons: HTMLButtonElement[] = $state([]);
  const index = $derived(options.findIndex((o) => o.value === value));

  function key(e: KeyboardEvent, i: number) {
    const n = options.length;
    let j = i;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") j = (i + 1) % n;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") j = (i - 1 + n) % n;
    else if (e.key === "Home") j = 0;
    else if (e.key === "End") j = n - 1;
    else return;
    e.preventDefault();
    onchange(options[j].value);
    queueMicrotask(() => buttons[j]?.focus());
  }
</script>

<div class="chips" role="radiogroup" aria-label={label}>
  {#each options as o, i (o.value)}
    <button
      bind:this={buttons[i]}
      type="button"
      role="radio"
      aria-checked={o.value === value}
      tabindex={i === (index < 0 ? 0 : index) ? 0 : -1}
      onclick={() => onchange(o.value)}
      onkeydown={(e) => key(e, i)}
    >
      <span class="main num">{o.label}</span>
      {#if o.sub}<span class="sub">{o.sub}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  button {
    display: inline-flex;
    align-items: baseline;
    gap: 5px;
    height: 32px;
    padding: 0 11px;
    border-radius: 999px;
    border: 1px solid var(--separator);
    background: var(--surface);
    font-size: 13px;
    font-weight: 560;
    color: var(--text);
    transition: background-color var(--t-fast) var(--ease-out), border-color var(--t-fast) var(--ease-out),
      color var(--t-fast), transform var(--t-med) var(--spring);
  }
  button:hover {
    border-color: var(--separator-strong);
  }
  button:active {
    transform: scale(0.95);
    transition-duration: 90ms;
  }
  button[aria-checked="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .sub {
    font-size: 11.5px;
    font-weight: 500;
    opacity: 0.7;
  }
  @media (pointer: coarse) {
    button {
      height: 40px;
      padding: 0 14px;
      font-size: 14px;
    }
  }
</style>
