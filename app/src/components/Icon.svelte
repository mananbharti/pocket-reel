<script lang="ts" module>
  // A small, consistent line-icon set drawn on a 24px grid with 1.8px strokes.
  const paths: Record<string, string> = {
    plus: "M12 5v14M5 12h14",
    film: "M7 4v16M17 4v16M3 12h4M17 12h4M3 8h4M17 8h4M3 16h4M17 16h4",
    music: "M9 17.5V6.2l10-2.2v11.3",
    tv: "M8 20.5h8M12 17v3.5",
    phone: "M11 18.2h2",
    disc: "",
    globe: "M3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3Z",
    folder: "M3.5 8a2 2 0 0 1 2-2h3.6c.5 0 1 .2 1.4.6l1.3 1.3c.4.4.9.6 1.4.6h5.3a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2Z",
    x: "M6.5 6.5l11 11M17.5 6.5l-11 11",
    check: "M5.2 12.6l4.3 4.2 9.3-9.6",
    chevronDown: "M6.5 9.5 12 15l5.5-5.5",
    chevronRight: "M9.5 6.5 15 12l-5.5 5.5",
    retry: "M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4h-4",
    trash: "M4.5 7h15M10 11v6M14 11v6M6.5 7l.8 11.2a2 2 0 0 0 2 1.8h5.4a2 2 0 0 0 2-1.8L17.5 7M9.5 7V5.5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1V7",
    scissors: "M8.3 8.6 19.5 17.8M8.3 15.4 19.5 6.2",
    sliders: "M4 7h9M17 7h3M4 12h3M11 12h9M4 17h11M19 17h1",
    sun: "M12 2.8v2M12 19.2v2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M2.8 12h2M19.2 12h2M4.6 19.4 6 18M18 6l1.4-1.4",
    moon: "M19.5 14.6A7.8 7.8 0 0 1 9.4 4.5 7.8 7.8 0 1 0 19.5 14.6Z",
    reveal: "M14 4.5h5.5V10M19.5 4.5 11 13M18 14v3.5a2 2 0 0 1-2 2H6.5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2H10",
    share: "M12 15V3.8M8 7.5l4-4 4 4M7 11H6a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-6a2 2 0 0 0-2-2h-1",
    info: "M12 11v5.5M12 7.6v.1",
    warning: "M12 9.5v4M12 17v.1M10.3 4.4 2.9 17.3A2 2 0 0 0 4.6 20.3h14.8a2 2 0 0 0 1.7-3L13.7 4.4a2 2 0 0 0-3.4 0Z",
    arrowDown: "M12 4.5v12M6.5 11l5.5 5.5 5.5-5.5M5 20h14",
    stop: "",
    play: "",
    contrast: "",
    auto: "",
    image: "M4 17l5-5 4 4 2.5-2.5L20 18",
    clock: "M12 7.5V12l3 2",
  };

  const extra: Record<string, string> = {
    film: '<rect x="3" y="4" width="18" height="16" rx="3"/>',
    music: '<circle cx="7" cy="17.5" r="2.2"/><circle cx="17" cy="15.3" r="2.2"/>',
    tv: '<rect x="3" y="4.5" width="18" height="12.5" rx="2.5"/>',
    phone: '<rect x="7" y="2.8" width="10" height="18.4" rx="2.6"/>',
    disc: '<circle cx="12" cy="12" r="8.6"/><circle cx="12" cy="12" r="2.4"/>',
    globe: '<circle cx="12" cy="12" r="9"/>',
    scissors: '<circle cx="6.2" cy="7" r="2.6"/><circle cx="6.2" cy="17" r="2.6"/>',
    sliders: '<circle cx="15" cy="7" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="17" cy="17" r="2"/>',
    sun: '<circle cx="12" cy="12" r="4"/>',
    info: '<circle cx="12" cy="12" r="9"/>',
    stop: '<rect x="7" y="7" width="10" height="10" rx="2.2" fill="currentColor" stroke="none"/>',
    play: '<path d="M8.5 5.8v12.4a.8.8 0 0 0 1.2.7l9.6-6.2a.8.8 0 0 0 0-1.4L9.7 5.1a.8.8 0 0 0-1.2.7Z" fill="currentColor" stroke="none"/>',
    contrast: '<circle cx="12" cy="12" r="8.6"/><path d="M12 3.4a8.6 8.6 0 0 1 0 17.2Z" fill="currentColor" stroke="none"/>',
    auto: '<circle cx="12" cy="12" r="8.6"/><path d="M12 3.4a8.6 8.6 0 0 0 0 17.2Z" fill="currentColor" stroke="none"/>',
    image: '<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><circle cx="9" cy="9.5" r="1.6"/>',
    clock: '<circle cx="12" cy="12" r="8.6"/>',
  };
</script>

<script lang="ts">
  interface Props {
    name: string;
    size?: number;
    stroke?: number;
    label?: string;
    class?: string;
  }
  let { name, size = 20, stroke = 1.8, label, class: cls = "" }: Props = $props();
</script>

<svg
  class="icon {cls}"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
  focusable="false"
>
  {#if extra[name]}{@html extra[name]}{/if}
  {#if paths[name]}<path d={paths[name]} />{/if}
</svg>

<style>
  .icon {
    flex: none;
    display: block;
  }
</style>
