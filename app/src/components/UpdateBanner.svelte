<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { updates } from "../lib/updates.svelte";
  import { bytes } from "../lib/format";
  import { springIn, collapseOut } from "../lib/motion";
  import Icon from "./Icon.svelte";
  import ProgressBar from "./ProgressBar.svelte";

  const u = updates;
  const busyConverting = $derived(app.active.length > 0);
  const pct = $derived(u.fraction == null ? null : Math.floor(u.fraction * 100));

  const title = $derived.by(() => {
    const v = u.info?.version ?? "";
    switch (u.phase) {
      case "available":
        return `Pocket Reel ${v} is available`;
      case "download":
        return pct == null ? "Downloading the update…" : `Downloading the update… ${pct}%`;
      case "verify":
        return "Checking the download…";
      case "install":
        return app.isMobile ? "Opening the installer…" : "Installing…";
      case "done":
        if (u.outcome?.outcome === "restarting") return "Restarting with the new version…";
        if (u.outcome?.outcome === "manual") return "Almost done";
        return `Install Pocket Reel ${v}`;
      case "error":
        return "The update didn't finish";
      default:
        return "";
    }
  });

  const detail = $derived.by(() => {
    switch (u.phase) {
      case "available":
        if (busyConverting) return "Finish converting first, then update.";
        return `You have ${app.info?.version ?? "an older version"}. Your videos and settings stay as they are.`;
      case "download":
        return u.total ? `${bytes(u.downloaded)} of ${bytes(u.total)}` : bytes(u.downloaded);
      case "verify":
        return "Making sure the file arrived intact.";
      case "install":
        return app.isMobile ? "" : "Pocket Reel will close and reopen by itself.";
      case "done":
        if (u.outcome?.outcome === "manual") return "Drag Pocket Reel into the Applications folder in the window that opened.";
        if (u.outcome?.outcome === "installer")
          return u.outcome.needsPermission
            ? "Allow Pocket Reel to install apps when asked, then tap Install (or Update)."
            : "Tap Install (or Update) on the screen that opened.";
        return "";
      case "error":
        return u.error ?? "";
      default:
        return "";
    }
  });
</script>

{#if u.visible}
  <section
    class="update"
    class:error={u.phase === "error"}
    in:springIn={{ y: -10 }}
    out:collapseOut
    aria-live="polite"
    aria-label="App update"
  >
    <div class="row">
      <span class="badge" class:spin={u.phase === "verify" || u.phase === "install"} aria-hidden="true">
        <Icon name={u.phase === "error" ? "warning" : u.phase === "done" ? "check" : "arrowDown"} size={18} stroke={2.2} />
      </span>
      <div class="text">
        <strong>{title}</strong>
        {#if detail}<span class="detail num">{detail}</span>{/if}
      </div>
      <div class="actions">
        {#if u.phase === "available"}
          <button type="button" class="btn plain" onclick={() => u.whatsNew()}>What’s new</button>
          <button type="button" class="btn plain" onclick={() => u.later()}>Later</button>
          <button type="button" class="btn primary" disabled={busyConverting} onclick={() => u.install()}>Update</button>
        {:else if u.phase === "error"}
          <button type="button" class="btn plain" onclick={() => u.later()}>Later</button>
          <button type="button" class="btn primary" disabled={busyConverting} onclick={() => u.install()}>
            <Icon name="retry" size={15} /> Try again
          </button>
        {:else if u.phase === "done" && u.outcome?.outcome === "installer"}
          <button type="button" class="btn plain" onclick={() => u.install()}>Open installer again</button>
        {/if}
      </div>
    </div>
    {#if u.phase === "download" || u.phase === "verify" || u.phase === "install"}
      <ProgressBar
        value={u.phase === "download" ? u.fraction : null}
        label="Update progress"
        valueText={title}
        stage={u.phase === "download" ? "download" : null}
      />
    {/if}
  </section>
{/if}

<style>
  .update {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 16px;
    padding: 14px 14px 14px 16px;
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow: var(--shadow-md);
    border: var(--border-width) solid var(--separator);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .badge {
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--accent);
    background: var(--accent-soft);
  }
  .error .badge {
    color: var(--danger);
    background: var(--danger-soft);
  }
  .badge.spin :global(svg) {
    animation: bob 1.1s var(--ease-in-out) infinite;
  }
  @keyframes bob {
    50% {
      transform: translateY(3px);
    }
  }
  .text {
    flex: 1 1 220px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  strong {
    font-size: 14.5px;
    font-weight: 650;
  }
  .detail {
    font-size: 13px;
    color: var(--text-2);
  }
  .error .detail {
    color: var(--danger);
  }
  .actions {
    display: flex;
    gap: 6px;
    margin-left: auto;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .actions :global(.btn) {
    gap: 6px;
  }
  @media (max-width: 560px) {
    .actions {
      width: 100%;
    }
    .actions .primary {
      flex: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .badge.spin :global(svg) {
      animation: none;
    }
  }
</style>
