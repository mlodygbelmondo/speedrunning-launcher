<script lang="ts">
  import Icon from './Icon.svelte';
  let { checked = $bindable(false), label }: { checked?: boolean; label?: string } = $props();
</script>

<button
  class="cb"
  class:on={checked}
  role="checkbox"
  aria-checked={checked}
  aria-label={label}
  onclick={() => (checked = !checked)}
>
  {#if checked}<Icon name="check" size={12} />{/if}
</button>

<style>
  .cb {
    flex: none;
    width: 18px;
    height: 18px;
    padding: 0;
    border-radius: 5px;
    border: 1.5px solid var(--border);
    background: transparent;
    color: var(--text);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  .cb.on {
    border: 0;
    background: var(--accent);
  }
</style>
