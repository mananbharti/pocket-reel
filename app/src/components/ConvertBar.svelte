<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { bytes } from "../lib/format";
  import { CLASSIC_SCREENS } from "../lib/screens";
  import Icon from "./Icon.svelte";

  const count = $derived(app.pending.length || app.convertible.length);
  const again = $derived(!app.pending.length && app.convertible.length > 0);
  const busy = $derived(app.active.length > 0 && app.pending.length === 0);
  const label = $derived(
    busy
      ? "Converting…"
      : count === 0
        ? "Convert"
        : again
          ? "Convert again"
          : `Convert ${count} ${count === 1 ? "video" : "videos"}`,
  );

  async function chooseFolder() {
    const dir = await app.api?.pickFolder();
    if (dir) app.setPref("outputDir", dir);
  }
  const folderName = $derived(app.prefs.outputDir?.split(/[\\/]/).filter(Boolean).pop() ?? "");
</script>

<div class="bar">
  <div class="opts">
    <label class="opt">
      <span class="lbl">Player screen</span>
      <select value={app.prefs.screen} onchange={(e) => app.setScreen(e.currentTarget.value)}>
        {#each CLASSIC_SCREENS as s (s)}
          <option value={s}>{s.replace("x", " × ")}{s === "160x120" ? " (most players)" : ""}</option>
        {/each}
      </select>
    </label>
    {#if app.api?.supportsFolders}
      <div class="opt">
        <span class="lbl" id="save-lbl">Save to</span>
        <div class="save">
          <button type="button" class="pick" aria-describedby="save-lbl" onclick={chooseFolder} title={app.prefs.outputDir ?? ""}>
            <Icon name="folder" size={17} />
            <span>{folderName || "Next to each video"}</span>
          </button>
          {#if app.prefs.outputDir}
            <button type="button" class="reset" aria-label="Save next to each video instead" onclick={() => app.setPref("outputDir", null)}>
              <Icon name="x" size={14} />
            </button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
  <div class="go-wrap">
    {#if app.pending.length && !app.estimatesPending}
      <span class="total num">about {bytes(app.totalEstimate)}</span>
    {/if}
    {#if app.active.length}
      <button type="button" class="btn large" onclick={() => app.cancelAll()}>
        <Icon name="stop" size={15} /> Stop
      </button>
    {/if}
    <button
      type="button"
      class="btn primary large go"
      disabled={busy || count === 0}
      aria-keyshortcuts="Control+Enter"
      onclick={() => app.convert()}
    >
      {label}
    </button>
  </div>
</div>

<style>
  .bar {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 14px 20px;
    padding: 14px max(20px, calc((100vw - 760px) / 2));
    padding-bottom: calc(14px + env(safe-area-inset-bottom) + var(--nav-bottom, 0px));
    background: var(--material);
    backdrop-filter: saturate(180%) blur(20px);
    -webkit-backdrop-filter: saturate(180%) blur(20px);
    border-top: 1px solid var(--separator);
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 18px;
    min-width: 0;
  }
  .opt {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .lbl {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  select,
  .pick {
    height: 40px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--separator-strong);
    background: var(--bg-elevated);
    font-size: 14.5px;
    font-weight: 560;
  }
  select {
    padding-right: 30px;
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%2386868b' stroke-width='2.2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M6.5 9.5 12 15l5.5-5.5'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 9px center;
    background-size: 16px;
  }
  .save {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .pick {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    max-width: 240px;
    color: var(--text);
  }
  .pick span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pick :global(svg) {
    color: var(--accent);
    flex: none;
  }
  .reset {
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text-3);
    display: grid;
    place-items: center;
  }
  .reset:hover {
    background: var(--fill);
  }
  .go-wrap {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-left: auto;
  }
  .total {
    font-size: 13px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .go {
    min-width: 190px;
  }
  @media (max-width: 560px) {
    .opts,
    .opt,
    select {
      width: 100%;
    }
    .go-wrap {
      width: 100%;
    }
    .go {
      flex: 1;
      min-width: 0;
    }
  }
</style>
