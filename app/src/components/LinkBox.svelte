<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  let value = $state("");
  let busy = $state(false);
  const id = `link-${Math.random().toString(36).slice(2, 7)}`;

  async function add(e?: Event) {
    e?.preventDefault();
    const text = value.trim();
    if (!text || busy) return;
    busy = true;
    const ok = await app.addLink(text);
    busy = false;
    if (ok) value = "";
  }

  async function pasteAndAdd() {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        value = text.trim();
        await add();
      }
    } catch {
      document.getElementById(id)?.focus();
    }
  }
</script>

<form class="link" onsubmit={add}>
  <label class="visually-hidden" for={id}>Video link</label>
  <span class="ico" aria-hidden="true"><Icon name="globe" size={18} /></span>
  <input
    {id}
    type="url"
    inputmode="url"
    autocomplete="off"
    spellcheck="false"
    placeholder="Paste a video link (YouTube, Instagram, any site)"
    bind:value
  />
  {#if value.trim()}
    <button type="submit" class="btn primary lgo" disabled={busy}>{busy ? "Adding…" : "Add"}</button>
  {:else}
    <button type="button" class="btn lgo" onclick={pasteAndAdd}>Paste</button>
  {/if}
</form>

<style>
  .link {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 6px 6px 12px;
    border-radius: var(--r-md);
    border: 1px solid var(--separator-strong);
    background: var(--bg-elevated);
    transition: border-color var(--t-fast), box-shadow var(--t-fast);
  }
  .link:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 var(--focus-width) var(--accent-ring);
  }
  .ico {
    color: var(--text-3);
    display: grid;
  }
  input {
    flex: 1;
    min-width: 0;
    height: 36px;
    border: 0;
    background: transparent;
    font-size: 15px;
  }
  input:focus-visible {
    outline: none;
  }
  .lgo {
    --h: 36px;
    flex: none;
    min-width: 76px;
  }
</style>
