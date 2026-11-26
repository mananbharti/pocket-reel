<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./lib/store.svelte";
  import { fadeScale } from "./lib/motion";
  import ConvertBar from "./components/ConvertBar.svelte";
  import DropZone from "./components/DropZone.svelte";
  import FileList from "./components/FileList.svelte";
  import Header from "./components/Header.svelte";
  import Icon from "./components/Icon.svelte";
  import Toasts from "./components/Toasts.svelte";

  onMount(() => {
    app.init().then(() => {
      document.documentElement.classList.toggle("mobile-app", app.isMobile);
    });
  });

  function keys(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "o") {
      e.preventDefault();
      app.pickFiles();
    } else if (mod && e.key === "Enter") {
      e.preventDefault();
      app.convert();
    }
  }
</script>

<svelte:window onkeydown={keys} />

<div class="shell">
  <Header />

  {#if app.fatal}
    <main class="center" role="alert">
      <Icon name="warning" size={36} />
      <h2>Pocket Reel can’t start</h2>
      <p>{app.fatal}</p>
    </main>
  {:else if !app.ready}
    <main class="center" aria-busy="true">
      <span class="spin" aria-label="Loading"></span>
    </main>
  {:else}
    <main class="content">
      <div class="column">
        {#if app.items.length === 0}
          <DropZone />
        {:else}
          <FileList />
        {/if}
      </div>
    </main>
    <ConvertBar />
  {/if}
</div>

{#if app.dragActive}
  <div class="drop" transition:fadeScale={{ start: 1.02 }} aria-hidden="true">
    <div class="drop-card">
      <Icon name="arrowDown" size={34} stroke={1.8} />
      <span>Drop to add</span>
    </div>
  </div>
{/if}

<Toasts />

<div class="visually-hidden" role="status" aria-live="polite" aria-atomic="true">{app.announcement}</div>

<style>
  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .column {
    max-width: 760px;
    min-height: 100%;
    margin: 0 auto;
    padding: 0 20px;
    display: flex;
    flex-direction: column;
  }
  .center {
    flex: 1;
    display: grid;
    place-content: center;
    justify-items: center;
    text-align: center;
    padding: 24px;
    color: var(--text-2);
  }
  .center h2 {
    color: var(--text);
    margin: 14px 0 6px;
  }
  .center p {
    max-width: 420px;
    margin: 0;
  }
  .spin {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 2.5px solid var(--fill-strong);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .drop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: var(--scrim-drop);
    backdrop-filter: blur(2px);
    pointer-events: none;
  }
  .drop-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 36px 54px;
    border-radius: 28px;
    border: 2.5px dashed var(--accent);
    background: var(--bg-elevated);
    color: var(--accent);
    font-size: 19px;
    font-weight: 680;
    box-shadow: var(--shadow-lg);
    animation: breathe 1.6s var(--ease-in-out) infinite;
  }
  @keyframes breathe {
    50% {
      transform: scale(1.025);
    }
  }
  @media (max-width: 560px) {
    .column {
      padding: 0 14px;
    }
  }
</style>
