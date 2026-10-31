<script lang="ts">
  // Fake install progress: fills over 1200 ms (linear), then the parent swaps it out.
  let { inset = 16 }: { inset?: number } = $props();
</script>

<div class="bar" style:left="{inset}px" style:right="{inset}px"><div class="fill"></div></div>

<style>
  .bar {
    position: absolute;
    bottom: 0;
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transform-origin: left;
    animation: fill 1200ms linear forwards;
  }
  @keyframes fill {
    from { transform: scaleX(0); }
    to { transform: scaleX(1); }
  }
</style>
