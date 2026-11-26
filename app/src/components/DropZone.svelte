<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
</script>

<section class="empty" aria-labelledby="empty-title">
  <div class="art" aria-hidden="true"><Icon name="film" size={40} stroke={1.5} /></div>
  <h2 id="empty-title">Add your videos</h2>
  <p>Pick any videos. Pocket Reel turns them into files your MP3 player can play.</p>
  <button type="button" class="btn primary large" onclick={() => app.pickFiles()}>
    <Icon name="plus" size={20} />
    Choose videos
  </button>
  {#if !app.isMobile}
    <p class="hint">or drop them here</p>
  {/if}
  <ol class="steps">
    <li><span class="n">1</span>Add videos</li>
    <li><span class="n">2</span>Press Convert</li>
    <li><span class="n">3</span>Copy them to your player</li>
  </ol>
</section>

<style>
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 40px 24px;
    margin: 20px 0;
    border-radius: var(--r-xl);
    border: 2px dashed var(--separator-strong);
    background: var(--surface);
    animation: rise var(--t-slow) var(--spring-soft) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.99);
    }
  }
  .art {
    width: 84px;
    height: 84px;
    border-radius: 22px;
    display: grid;
    place-items: center;
    background: var(--accent-soft);
    color: var(--accent);
  }
  h2 {
    margin: 20px 0 6px;
    font-family: var(--font-display);
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  p {
    margin: 0 0 24px;
    max-width: 380px;
    color: var(--text-2);
    font-size: 16px;
    line-height: 1.5;
  }
  .hint {
    margin: 12px 0 0;
    font-size: 13.5px;
    color: var(--text-3);
  }
  .steps {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px 22px;
    margin: 34px 0 0;
    padding: 0;
    font-size: 14px;
    color: var(--text-2);
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .n {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--fill);
    color: var(--text);
    font-size: 12px;
    font-weight: 700;
  }
</style>
