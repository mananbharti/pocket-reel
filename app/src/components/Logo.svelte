<script lang="ts">
  let { size = 28 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true" focusable="false">
  <defs>
    <linearGradient id="pr-bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#3b82f6" />
      <stop offset="1" stop-color="#1d4fd8" />
    </linearGradient>
  </defs>
  <rect width="512" height="512" rx="114" fill="url(#pr-bg)" />
  <rect x="104" y="140" width="304" height="232" rx="40" fill="#fff" />
  <g fill="#2563eb">
    {#each [138, 190, 242, 294, 346] as x (x)}
      <rect {x} y="160" width="28" height="18" rx="6" />
      <rect {x} y="334" width="28" height="18" rx="6" />
    {/each}
    <path d="M232 222.5v67a6 6 0 0 0 9.2 5.1l53.6-33.5a6 6 0 0 0 0-10.2l-53.6-33.5a6 6 0 0 0-9.2 5.1Z" />
  </g>
</svg>
