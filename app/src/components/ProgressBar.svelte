<script lang="ts">
  interface Props {
    value: number | null;
    label: string;
    valueText?: string;
    tone?: "accent" | "success" | "danger";
  }
  let { value, label, valueText, tone = "accent" }: Props = $props();
  const pct = $derived(value == null ? null : Math.round(Math.min(1, Math.max(0, value)) * 1000) / 10);
</script>

<div
  class="bar {tone}"
  class:indeterminate={pct == null}
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={pct ?? undefined}
  aria-valuetext={valueText}
>
  <span class="fill" style="transform: scaleX({pct == null ? 0.3 : pct / 100})"></span>
</div>

<style>
  .bar {
    position: relative;
    height: 6px;
    border-radius: 999px;
    background: var(--fill);
    overflow: hidden;
  }
  .fill {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--accent);
    transform-origin: left center;
    transition: transform 500ms var(--ease-out), background-color var(--t-med);
  }
  .success .fill {
    background: var(--success);
  }
  .danger .fill {
    background: var(--danger);
  }
  .indeterminate .fill {
    animation: slide 1.4s var(--ease-in-out) infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%) scaleX(0.3);
    }
    to {
      transform: translateX(110%) scaleX(0.3);
    }
  }
</style>
