<script lang="ts">
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";

  interface Props {
    value: number | null;
    label: string;
    valueText?: string;
    tone?: "accent" | "success" | "danger";
    /** "download" shows moving stripes, "convert" a travelling sheen. */
    stage?: "download" | "convert" | null;
    size?: "small" | "large";
  }
  let { value, label, valueText, tone = "accent", stage = null, size = "small" }: Props = $props();

  // Updates arrive a few times a second (every 500 ms on Android); glide between them.
  const shown = new Tween(0, { duration: 600, easing: cubicOut });
  $effect(() => {
    const v = value == null ? 0 : Math.min(1, Math.max(0, value));
    // Never animate backwards slowly (a new stage starts at 0): jump instead.
    shown.set(v, v < shown.current - 0.02 ? { duration: 0 } : undefined);
  });
  const pct = $derived(value == null ? null : Math.round(Math.min(1, Math.max(0, value)) * 1000) / 10);
</script>

<div
  class="bar {tone} {size}"
  class:indeterminate={pct == null}
  class:download={stage === "download"}
  class:convert={stage === "convert"}
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={pct ?? undefined}
  aria-valuetext={valueText}
>
  <span class="fill" style="width: {pct == null ? 30 : shown.current * 100}%"></span>
</div>

<style>
  .bar {
    position: relative;
    height: 6px;
    border-radius: 999px;
    background: var(--fill);
    overflow: hidden;
  }
  .bar.large {
    height: 8px;
  }
  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    min-width: 6px;
    border-radius: inherit;
    background: var(--accent);
    transition: background-color var(--t-med);
    overflow: hidden;
  }
  .success .fill {
    background: var(--success);
  }
  .danger .fill {
    background: var(--danger);
  }
  /* Downloading: diagonal stripes drifting along. */
  .download .fill {
    background-image: repeating-linear-gradient(
      -45deg,
      rgba(255, 255, 255, 0.28) 0 6px,
      transparent 6px 12px
    );
    background-size: 17px 17px;
    animation: stripes 0.7s linear infinite;
  }
  @keyframes stripes {
    to {
      background-position: 17px 0;
    }
  }
  /* Converting: a soft highlight sweeping across the filled part. */
  .convert .fill::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.45), transparent);
    transform: translateX(-100%);
    animation: sheen 1.6s var(--ease-in-out, ease-in-out) infinite;
  }
  @keyframes sheen {
    to {
      transform: translateX(100%);
    }
  }
  .indeterminate .fill {
    animation: slide 1.4s var(--ease-in-out, ease-in-out) infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .download .fill,
    .convert .fill::after {
      animation: none;
    }
  }
</style>
