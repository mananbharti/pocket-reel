<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
</script>

<section class="empty" aria-labelledby="empty-title">
  <div class="art" aria-hidden="true">
    <div class="card c1"><Icon name="music" size={26} stroke={1.6} /></div>
    <div class="card c2"><Icon name="film" size={30} stroke={1.6} /></div>
    <div class="card c3"><Icon name="tv" size={26} stroke={1.6} /></div>
  </div>
  <h2 id="empty-title">{app.isMobile ? "Choose videos to convert" : "Drop videos here"}</h2>
  <p>
    Convert MP4 and almost any other video for classic MP3/MP4 players, phones, TVs and the web, or pull out the
    music as MP3.
  </p>
  <button type="button" class="btn primary large" onclick={() => app.pickFiles()}>
    <Icon name="plus" size={20} />
    Choose Videos…
  </button>
  {#if !app.isMobile}
    <p class="hint">or press <kbd>Ctrl</kbd> <kbd>O</kbd></p>
  {/if}
</section>

<style>
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 40px 28px;
    margin: 16px;
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
  h2 {
    margin: 22px 0 6px;
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  p {
    margin: 0 0 22px;
    max-width: 420px;
    color: var(--text-2);
    font-size: 15px;
    line-height: 1.5;
  }
  .hint {
    margin: 14px 0 0;
    font-size: 13px;
    color: var(--text-3);
  }
  kbd {
    font-family: var(--font);
    font-size: 12px;
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--separator-strong);
    background: var(--surface-2);
  }
  .art {
    position: relative;
    width: 180px;
    height: 96px;
  }
  .card {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 76px;
    height: 76px;
    margin: -38px 0 0 -38px;
    border-radius: 20px;
    display: grid;
    place-items: center;
    background: var(--bg-elevated);
    border: var(--border-width) solid var(--separator);
    box-shadow: var(--shadow-md);
    color: var(--text-2);
    transition: transform var(--t-slow) var(--spring);
  }
  .c1 {
    transform: translateX(-58px) rotate(-10deg) scale(0.86);
  }
  .c3 {
    transform: translateX(58px) rotate(10deg) scale(0.86);
  }
  .c2 {
    z-index: 1;
    width: 88px;
    height: 88px;
    margin: -44px 0 0 -44px;
    color: var(--accent);
  }
  .empty:hover .c1 {
    transform: translateX(-66px) rotate(-14deg) scale(0.88);
  }
  .empty:hover .c3 {
    transform: translateX(66px) rotate(14deg) scale(0.88);
  }
  .empty:hover .c2 {
    transform: translateY(-4px);
  }
  @media (max-width: 760px) {
    .empty {
      margin: 12px;
      padding: 32px 20px;
    }
    h2 {
      font-size: 21px;
    }
  }
</style>
