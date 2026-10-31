<script lang="ts">
  let {
    items,
    value = $bindable(0),
    onchange,
  }: { items: string[]; value?: number; onchange?: (i: number) => void } = $props();
</script>

<div class="seg" role="tablist">
  {#each items as label, i (i)}
    <button
      class:sel={i === value}
      role="tab"
      aria-selected={i === value}
      onclick={() => {
        if (i === value) return;
        value = i;
        onchange?.(i);
      }}>{label}</button
    >
  {/each}
</div>

<style>
  .seg {
    flex: none;
    display: inline-flex;
    padding: 3px;
    border-radius: 10px;
    background: var(--raised);
  }
  button {
    height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
    font: 500 13px/1 'Inter', sans-serif;
    white-space: nowrap;
    cursor: pointer;
  }
  button:hover { color: var(--text); }
  button.sel {
    background: var(--accent);
    color: var(--text);
  }
</style>
