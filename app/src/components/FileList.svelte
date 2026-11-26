<script lang="ts">
  import { flip } from "svelte/animate";
  import { app } from "../lib/store.svelte";
  import { springIn, collapseOut } from "../lib/motion";
  import FileRow from "./FileRow.svelte";
  import LinkBox from "./LinkBox.svelte";

  const doneCount = $derived(app.items.filter((i) => app.jobFor(i)?.state === "done").length);
</script>

<section class="files" aria-labelledby="files-title">
  <div class="head">
    <h2 id="files-title">{app.items.length} {app.items.length === 1 ? "video" : "videos"}</h2>
    {#if doneCount}
      <button type="button" class="btn plain small" onclick={() => app.clearFinished()}>Clear finished</button>
    {/if}
  </div>
  <ul>
    {#each app.items as item (item.key)}
      <li animate:flip={{ duration: 380, easing: (t) => 1 - Math.pow(1 - t, 4) }} in:springIn out:collapseOut>
        <FileRow {item} />
      </li>
    {/each}
  </ul>
  {#if app.canDownload}
    <div class="linkrow"><LinkBox /></div>
  {/if}
  <button type="button" class="more" onclick={() => app.pickFiles()}>
    <span class="plus" aria-hidden="true">+</span> Add more videos
  </button>
</section>

<style>
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 4px 8px;
  }
  h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-2);
  }
  .small {
    --h: 28px;
    font-size: 13px;
    color: var(--accent);
    padding: 0 10px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px 0 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .linkrow {
    margin-bottom: 10px;
  }
  .more {
    width: 100%;
    height: 56px;
    margin-bottom: 24px;
    border-radius: var(--r-lg);
    border: 1.5px dashed var(--separator-strong);
    background: transparent;
    color: var(--text-2);
    font-weight: 600;
    font-size: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    transition: border-color var(--t-fast), color var(--t-fast), background-color var(--t-fast);
  }
  .more:hover {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }
  .plus {
    font-size: 20px;
    font-weight: 400;
    line-height: 1;
  }
</style>
