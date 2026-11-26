<script lang="ts">
  import { flip } from "svelte/animate";
  import { app } from "../lib/store.svelte";
  import { springIn, collapseOut } from "../lib/motion";
  import FileRow from "./FileRow.svelte";

  const doneCount = $derived(app.items.filter((i) => app.jobFor(i)?.state === "done").length);
  let list: HTMLUListElement | undefined = $state();

  function navigate(from: number, dir: 1 | -1) {
    const i = Math.min(app.items.length - 1, Math.max(0, from + dir));
    const key = app.items[i]?.key;
    if (!key) return;
    app.select(key);
    list?.querySelector<HTMLButtonElement>(`button.select[data-key="${key}"]`)?.focus();
  }
</script>

<section class="files" aria-labelledby="files-title">
  <div class="head">
    <h2 id="files-title">
      {app.items.length} {app.items.length === 1 ? "file" : "files"}
    </h2>
    {#if doneCount}
      <button type="button" class="btn plain small" onclick={() => app.clearFinished()}>Clear finished</button>
    {/if}
  </div>
  <ul bind:this={list}>
    {#each app.items as item, i (item.key)}
      <li animate:flip={{ duration: 380, easing: (t) => 1 - Math.pow(1 - t, 4) }} in:springIn out:collapseOut>
        <FileRow {item} onnavigate={(d) => navigate(i, d)} />
      </li>
    {/each}
  </ul>
  <button type="button" class="more" onclick={() => app.pickFiles()}>
    <span class="plus" aria-hidden="true">+</span>
    {app.isMobile ? "Add more videos" : "Add more, or drop files anywhere"}
  </button>
</section>

<style>
  .files {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 20px 8px;
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
    padding: 4px 16px 24px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .more {
    margin: 0 16px 28px;
    height: 54px;
    border-radius: var(--r-lg);
    border: 1.5px dashed var(--separator-strong);
    background: transparent;
    color: var(--text-2);
    font-weight: 560;
    font-size: 13.5px;
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
    font-size: 18px;
    font-weight: 400;
    line-height: 1;
  }
  @media (max-width: 760px) {
    .more {
      margin: 0 12px 16px;
    }
    ul {
      padding: 4px 12px 16px;
    }
  }
</style>
