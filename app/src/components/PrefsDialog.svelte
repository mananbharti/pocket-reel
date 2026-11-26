<script lang="ts">
  import { app, type Theme } from "../lib/store.svelte";
  import Dialog from "./Dialog.svelte";
  import Segmented from "./Segmented.svelte";
  import Switch from "./Switch.svelte";

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
  const mod = navigator.platform.toLowerCase().includes("mac") ? "⌘" : "Ctrl";
</script>

<Dialog {open} {onclose} title="Preferences" width={480}>
  <div class="sec">
    <span class="label" id="appearance">Appearance</span>
    <Segmented
      label="Appearance"
      options={[
        { value: "auto", label: "Auto" },
        { value: "light", label: "Light" },
        { value: "dark", label: "Dark" },
      ] as { value: Theme; label: string }[]}
      value={app.prefs.theme}
      onchange={(t) => app.setPref("theme", t)}
    />
    <p class="help">Auto follows your system setting.</p>
  </div>

  <div class="sec card">
    <Switch
      label="Increase contrast"
      description="Stronger borders and text for easier reading."
      checked={app.prefs.contrast === "high"}
      onchange={(v) => app.setPref("contrast", v ? "high" : "normal")}
    />
    <hr />
    <Switch
      label="Reduce motion"
      description="Replaces movement with simple fades. Also follows your system setting."
      checked={app.prefs.motion === "reduce"}
      onchange={(v) => app.setPref("motion", v ? "reduce" : "system")}
    />
  </div>

  {#if !app.isMobile}
    <div class="sec">
      <span class="label">Convert at the same time</span>
      <Segmented
        label="Files converted at the same time"
        options={[1, 2, 3, 4].map((n) => ({ value: n, label: String(n) }))}
        value={app.prefs.concurrency}
        onchange={(n) => app.setPref("concurrency", n)}
      />
      <p class="help">More is faster on computers with many cores.</p>
    </div>

    <div class="sec">
      <span class="label">Keyboard</span>
      <dl class="keys">
        <div><dt>Add videos</dt><dd><kbd>{mod}</kbd> <kbd>O</kbd></dd></div>
        <div><dt>Convert</dt><dd><kbd>{mod}</kbd> <kbd>Enter</kbd></dd></div>
        <div><dt>Move between files</dt><dd><kbd>↑</kbd> <kbd>↓</kbd></dd></div>
        <div><dt>Remove file</dt><dd><kbd>Delete</kbd></dd></div>
        <div><dt>Close dialog</dt><dd><kbd>Esc</kbd></dd></div>
      </dl>
    </div>
  {/if}

  <div class="about">
    <p><strong>Pocket Reel</strong> {app.info?.version ?? ""}</p>
    <p class="muted">Powered by {(app.info?.ffmpeg ?? "").match(/ffmpeg version (\S+)/i)?.[1]?.replace(/-.*$/, "").replace(/^n/, "FFmpeg ").replace(/^(\d)/, "FFmpeg $1") ?? "FFmpeg"}</p>
    <p class="muted">Free and open source under the GPL-3.0 license.</p>
  </div>
</Dialog>

<style>
  .sec + .sec {
    margin-top: 20px;
  }
  .label {
    display: block;
    font-weight: 620;
    margin-bottom: 8px;
    font-size: 14px;
  }
  .help {
    margin: 6px 0 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .card {
    padding: 14px 16px;
    border-radius: var(--r-lg);
    background: var(--surface-2);
    border: var(--border-width) solid var(--separator);
  }
  hr {
    border: 0;
    border-top: 1px solid var(--separator);
    margin: 12px 0;
  }
  .keys {
    margin: 0;
    display: grid;
    gap: 6px;
  }
  .keys div {
    display: flex;
    justify-content: space-between;
    font-size: 13.5px;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
  }
  kbd {
    font-family: var(--font);
    font-size: 12px;
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--separator-strong);
    background: var(--surface-2);
  }
  .about {
    margin-top: 22px;
    padding-top: 14px;
    border-top: 1px solid var(--separator);
    font-size: 13px;
  }
  .about p {
    margin: 0 0 3px;
  }
  .muted {
    color: var(--text-2);
    font-size: 12px;
    word-break: break-word;
  }
</style>
