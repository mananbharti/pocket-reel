<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { presetIcon } from "../lib/presetIcon";
  import AudioOptions from "./AudioOptions.svelte";
  import Icon from "./Icon.svelte";
  import OutputPreview from "./OutputPreview.svelte";
  import PresetPicker from "./PresetPicker.svelte";
  import VideoOptions from "./VideoOptions.svelte";

  let pickerOpen = $state(false);
  const p = $derived(app.preset);

  async function chooseFolder() {
    const dir = await app.api?.pickFolder();
    if (dir) app.setPref("outputDir", dir);
  }
</script>

<div class="panel" id="settings" tabindex="-1" aria-label="Conversion settings">
  <section class="group format" aria-labelledby="fmt-title">
    <h3 id="fmt-title">Convert to</h3>
    {#if p}
      <button type="button" class="preset" aria-haspopup="dialog" onclick={() => (pickerOpen = true)}>
        <span class="ico"><Icon name={presetIcon(p)} size={24} stroke={1.6} /></span>
        <span class="txt">
          <span class="name"><span class="nm">{p.name}</span>{#if p.original}<span class="badge">Original</span>{/if}</span>
          <span class="fmt">{p.formatLabel}</span>
        </span>
        <span class="chev"><Icon name="chevronRight" size={18} /></span>
        <span class="visually-hidden">. Change format</span>
      </button>
    {/if}
  </section>

  {#if app.selected}
    {#key app.selected.key}
      <div class="swap"><OutputPreview item={app.selected} /></div>
    {/key}
  {/if}

  {#if p?.video}
    {#key p.id}<div class="swap"><VideoOptions /></div>{/key}
  {/if}
  {#if p?.audio}
    {#key p.id}<div class="swap"><AudioOptions /></div>{/key}
  {/if}

  <section class="group" aria-labelledby="save-title">
    <h3 id="save-title">Save to</h3>
    {#if app.api?.supportsFolders}
      <div class="save">
        <Icon name="folder" size={20} stroke={1.7} />
        <span class="path" title={app.prefs.outputDir ?? ""}>
          {app.prefs.outputDir ?? "Same folder as each original"}
        </span>
        <button type="button" class="btn small" onclick={chooseFolder}>Change…</button>
        {#if app.prefs.outputDir}
          <button type="button" class="btn small plain" onclick={() => app.setPref("outputDir", null)}>Reset</button>
        {/if}
      </div>
    {:else if app.isMobile}
      <div class="save">
        <Icon name="folder" size={20} stroke={1.7} />
        <span class="path">{p?.category === "audio" ? "Music" : "Movies"} › Pocket Reel</span>
      </div>
    {:else}
      <div class="save">
        <Icon name="folder" size={20} stroke={1.7} />
        <span class="path">{app.info?.defaultOutputDir ?? "Downloads"}</span>
      </div>
    {/if}
  </section>
</div>

<PresetPicker open={pickerOpen} onclose={() => (pickerOpen = false)} />

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 16px 24px;
    outline: none;
  }
  .preset {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 13px;
    width: 100%;
    padding: 10px 10px 10px 10px;
    margin: -4px 0 -6px;
    border: 0;
    border-radius: var(--r-md);
    background: var(--surface-2);
    text-align: left;
    transition: background-color var(--t-fast), transform var(--t-med) var(--spring);
  }
  .preset:hover {
    background: var(--fill-strong);
  }
  .preset:active {
    transform: scale(0.985);
  }
  .ico {
    width: 44px;
    height: 44px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--accent-text);
    box-shadow: 0 4px 12px rgba(0, 113, 227, 0.25);
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    font-weight: 650;
    font-size: 15.5px;
    letter-spacing: -0.01em;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    column-gap: 8px;
    row-gap: 2px;
  }
  .nm {
    white-space: nowrap;
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
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chev {
    color: var(--text-3);
  }
  .swap {
    animation: swap var(--t-slow) var(--spring-soft);
  }
  @keyframes swap {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  .save {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-2);
  }
  .path {
    flex: 1;
    min-width: 0;
    color: var(--text);
    font-size: 13.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    direction: rtl;
    text-align: left;
  }
  .small {
    --h: 30px;
    font-size: 13px;
    padding: 0 11px;
  }
</style>
