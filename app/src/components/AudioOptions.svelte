<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { hz } from "../lib/format";
  import ChipGroup from "./ChipGroup.svelte";
  import Segmented from "./Segmented.svelte";

  const a = $derived(app.preset!.audio!);
  const eff = $derived(app.effective);
  const NAMES: Record<string, string> = {
    "classic-adpcm": "IMA ADPCM",
    "amv-adpcm": "IMA ADPCM (AMV)",
    aac: "AAC",
    mp3: "MP3",
    opus: "Opus",
    pcm: "16-bit PCM",
    flac: "FLAC",
  };
  const sourceHasAudio = $derived(app.selected?.media ? !!app.selected.media.audio : true);
</script>

<section class="group" aria-labelledby="audio-title">
  <h3 id="audio-title">Audio</h3>
  {#if !a.editable}
    <p class="fixed num">
      <strong>{NAMES[a.codec]}</strong> · {hz(a.sampleRate)} · {a.channels === 1 ? "Mono" : "Stereo"}
    </p>
    <p class="help">Fixed by this format so classic players can decode it.{#if !sourceHasAudio} This video has no sound, so a silent track is added.{/if}</p>
  {:else}
    {#if a.bitrateKbps != null}
      <div class="field">
        <span class="label">Bitrate</span>
        <ChipGroup
          label="Audio bitrate"
          options={a.bitrateOptions.map((k) => ({ value: k, label: `${k}`, sub: "kbps" }))}
          value={eff.audioBitrate}
          onchange={(k) => app.update({ audioBitrateKbps: k })}
        />
      </div>
    {:else}
      <p class="fixed"><strong>{NAMES[a.codec]}</strong> · lossless</p>
    {/if}
    {#if a.sampleRates.length > 1}
      <div class="field">
        <span class="label">Sample rate</span>
        <Segmented
          label="Sample rate"
          size="sm"
          options={a.sampleRates.map((r) => ({ value: r, label: hz(r) }))}
          value={eff.sampleRate}
          onchange={(r) => app.update({ sampleRate: r })}
        />
      </div>
    {/if}
    <div class="field">
      <span class="label">Channels</span>
      <Segmented
        label="Channels"
        options={[
          { value: 1, label: "Mono" },
          { value: 2, label: "Stereo" },
        ]}
        value={eff.channels}
        onchange={(c) => app.update({ channels: c })}
      />
    </div>
    {#if !sourceHasAudio}
      <p class="help warn">The selected video has no sound.</p>
    {/if}
  {/if}
</section>

<style>
  .fixed {
    margin: 0;
    font-size: 14px;
  }
  .warn {
    color: var(--warning) !important;
  }
</style>
