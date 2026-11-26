<script lang="ts">
  interface Props {
    checked: boolean;
    label: string;
    description?: string;
    onchange: (v: boolean) => void;
  }
  let { checked, label, description, onchange }: Props = $props();
  const id = `sw-${Math.random().toString(36).slice(2, 8)}`;
</script>

<div class="row">
  <div class="text">
    <span id="{id}-l" class="label">{label}</span>
    {#if description}<span id="{id}-d" class="desc">{description}</span>{/if}
  </div>
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-labelledby="{id}-l"
    aria-describedby={description ? `${id}-d` : undefined}
    class="switch"
    onclick={() => onchange(!checked)}
  >
    <span class="knob"></span>
  </button>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .label {
    font-weight: 560;
  }
  .desc {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .switch {
    flex: none;
    position: relative;
    width: 46px;
    height: 28px;
    border-radius: 999px;
    border: var(--border-width) solid var(--separator);
    background: var(--fill-strong);
    padding: 0;
    transition: background-color var(--t-med) var(--ease-out);
  }
  .switch[aria-checked="true"] {
    background: var(--success);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2), 0 0 0 0.5px rgba(0, 0, 0, 0.06);
    transition: transform var(--t-slow) var(--spring), width var(--t-med) var(--spring);
  }
  .switch:active .knob {
    width: 28px;
  }
  .switch[aria-checked="true"] .knob {
    transform: translateX(18px);
  }
  .switch[aria-checked="true"]:active .knob {
    transform: translateX(14px);
  }
</style>
