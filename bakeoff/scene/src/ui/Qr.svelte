<script lang="ts">
  // QR from screens.json rows of 0/1, on a white tile with a 4-module quiet zone.
  let { rows, module = 4.5 }: { rows: string[]; module?: number } = $props();
  const n = $derived(rows.length);
  const d = $derived(
    rows
      .map((r, y) => {
        let s = '';
        for (let x = 0; x < r.length; x++) if (r[x] === '1') s += `M${x} ${y}h1v1h-1z`;
        return s;
      })
      .join(''),
  );
</script>

<div class="qr" style:padding="{4 * module}px">
  <svg width={n * module} height={n * module} viewBox="0 0 {n} {n}" shape-rendering="crispEdges">
    <path {d} fill="#0A090E" />
  </svg>
</div>

<style>
  .qr {
    flex: none;
    background: #fff;
    border-radius: 10px;
    line-height: 0;
  }
  svg { display: block; }
</style>
