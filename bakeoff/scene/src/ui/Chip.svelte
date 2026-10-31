<script lang="ts">
  let { label, on = false, onclick }: { label: string; on?: boolean; onclick?: () => void } = $props();
</script>

<button class="chip" class:on {onclick}>{label}</button>

<style>
  .chip {
    flex: none;
    height: 30px;
    padding: 0 12px;
    border-radius: 15px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
    font: 500 13px/1 'Inter', sans-serif;
    cursor: pointer;
  }
  .chip:hover { color: var(--text); }
  .chip.on {
    background: rgb(139 92 246 / 0.18);
    border-color: var(--accent);
    color: var(--text);
  }
</style>
