<script lang="ts" generics="T extends string | number">
  // An accessible segmented control (ARIA radio group) with a sliding thumb.
  interface Option {
    value: T;
    label: string;
    hint?: string;
  }
  interface Props {
    options: Option[];
    value: T;
    label: string;
    onchange: (v: T) => void;
    size?: "sm" | "md";
    disabled?: boolean;
  }
  let { options, value, label, onchange, size = "md", disabled = false }: Props = $props();

  let buttons: HTMLButtonElement[] = $state([]);
  let thumb = $state({ left: 0, width: 0, ready: false });

  const index = $derived(Math.max(0, options.findIndex((o) => o.value === value)));

  function measure() {
    const b = buttons[index];
    if (b) thumb = { left: b.offsetLeft, width: b.offsetWidth, ready: true };
  }

  $effect(() => {
    void index;
    void options.length;
    measure();
  });

  $effect(() => {
    const ro = new ResizeObserver(measure);
    for (const b of buttons) if (b) ro.observe(b);
    return () => ro.disconnect();
  });

  function key(e: KeyboardEvent) {
    const n = options.length;
    let i = index;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") i = (i + 1) % n;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") i = (i - 1 + n) % n;
    else if (e.key === "Home") i = 0;
    else if (e.key === "End") i = n - 1;
    else return;
    e.preventDefault();
    onchange(options[i].value);
    queueMicrotask(() => buttons[i]?.focus());
  }
</script>

<div class="seg {size}" role="radiogroup" aria-label={label} aria-disabled={disabled || undefined}>
  <span
    class="thumb"
    class:ready={thumb.ready}
    style="transform: translateX({thumb.left}px); width: {thumb.width}px"
    aria-hidden="true"
  ></span>
  {#each options as o, i (o.value)}
    <button
      bind:this={buttons[i]}
      type="button"
      role="radio"
      aria-checked={o.value === value}
      tabindex={o.value === value ? 0 : -1}
      title={o.hint}
      {disabled}
      onclick={() => onchange(o.value)}
      onkeydown={key}
    >
      {o.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    position: relative;
    display: flex;
    padding: 2px;
    border-radius: var(--r-sm);
    background: var(--fill);
    border: var(--border-width) solid var(--separator);
    isolation: isolate;
  }
  .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 0;
    border-radius: 7px;
    background: var(--bg-elevated);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12), 0 0 0 0.5px rgba(0, 0, 0, 0.06);
    z-index: -1;
    opacity: 0;
  }
  :global([data-contrast="high"]) .thumb {
    outline: 2px solid var(--accent);
  }
  .thumb.ready {
    opacity: 1;
    transition: transform var(--t-slow) var(--spring), width var(--t-slow) var(--spring), opacity var(--t-fast);
  }
  button {
    flex: 1 1 0;
    min-width: 0;
    height: 30px;
    padding: 0 10px;
    border: 0;
    background: transparent;
    border-radius: 7px;
    font-size: 13px;
    font-weight: 560;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color var(--t-fast) var(--ease-out);
  }
  button[aria-checked="true"] {
    color: var(--text);
  }
  button:hover:not([aria-checked="true"]):not(:disabled) {
    color: var(--text);
  }
  .sm button {
    height: 26px;
    font-size: 12.5px;
  }
  @media (pointer: coarse) {
    button {
      height: 40px;
      font-size: 14px;
    }
  }
</style>
