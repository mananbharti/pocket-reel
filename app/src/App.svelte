<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./lib/store.svelte";
  import { fadeScale } from "./lib/motion";
  import ConvertBar from "./components/ConvertBar.svelte";
  import DropZone from "./components/DropZone.svelte";
  import FileList from "./components/FileList.svelte";
  import Header from "./components/Header.svelte";
  import Icon from "./components/Icon.svelte";
  import PrefsDialog from "./components/PrefsDialog.svelte";
  import SettingsPanel from "./components/SettingsPanel.svelte";
  import Toasts from "./components/Toasts.svelte";

  let prefsOpen = $state(false);

  onMount(() => {
    app.init();
  });

  function keys(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "o") {
      e.preventDefault();
      app.pickFiles();
    } else if (mod && e.key === "Enter") {
      e.preventDefault();
      app.convert();
    } else if (mod && e.key === ",") {
      e.preventDefault();
      prefsOpen = true;
    }
  }
</script>

<svelte:window onkeydown={keys} />

<a class="skip" href="#settings">Skip to conversion settings</a>

<div class="shell" class:mobile={app.isMobile}>
  <Header onprefs={() => (prefsOpen = true)} />

  {#if app.fatal}
    <main class="fatal" role="alert">
      <Icon name="warning" size={36} />
      <h2>Pocket Reel can’t start</h2>
      <p>{app.fatal}</p>
    </main>
  {:else if !app.ready}
    <main class="loading" aria-busy="true">
      <span class="spin" aria-label="Loading"></span>
    </main>
  {:else}
    <main class="layout">
      <div class="left">
        {#if app.items.length === 0}
          <DropZone />
        {:else}
          <FileList />
        {/if}
      </div>
      <aside class="right" aria-label="Settings">
        <div class="scroll"><SettingsPanel /></div>
        <ConvertBar />
      </aside>
    </main>
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
<PrefsDialog open={prefsOpen} onclose={() => (prefsOpen = false)} />

<div class="visually-hidden" role="status" aria-live="polite" aria-atomic="true">{app.announcement}</div>

<style>
  .skip {
    position: fixed;
    left: 12px;
    top: -60px;
    z-index: 100;
    padding: 10px 14px;
    border-radius: var(--r-sm);
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 600;
    text-decoration: none;
    transition: top var(--t-med) var(--spring);
  }
  .skip:focus {
    top: 12px;
  }
  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .layout {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 400px;
  }
  .left {
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .right {
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border-left: 1px solid var(--separator);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .loading,
  .fatal {
    flex: 1;
    display: grid;
    place-content: center;
    justify-items: center;
    text-align: center;
    padding: 24px;
    color: var(--text-2);
  }
  .fatal h2 {
    color: var(--text);
    margin: 14px 0 6px;
  }
  .fatal p {
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
  @media (max-width: 860px) {
    .layout {
      grid-template-columns: minmax(0, 1fr) 360px;
    }
  }
  @media (max-width: 760px) {
    .layout {
      display: flex;
      flex-direction: column;
      overflow-y: auto;
    }
    .left {
      overflow: visible;
      flex: none;
      min-height: auto;
    }
    .left :global(.empty) {
      min-height: 360px;
    }
    .right {
      flex: none;
      border-left: 0;
      min-height: auto;
    }
    .scroll {
      overflow: visible;
    }
    .right :global(.bar) {
      position: sticky;
      bottom: 0;
      z-index: 4;
    }
  }
</style>
