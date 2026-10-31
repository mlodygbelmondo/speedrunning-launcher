<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let {
    variant = 'secondary',
    small = false,
    icon,
    iconAfter,
    spin = false,
    disabled = false,
    title,
    onclick,
    children,
  }: {
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
    small?: boolean;
    icon?: string;
    iconAfter?: string;
    spin?: boolean;
    disabled?: boolean;
    title?: string;
    onclick?: (e: MouseEvent) => void;
    children?: Snippet;
  } = $props();
</script>

<button class="btn {variant}" class:small class:disabled {disabled} {title} {onclick}>
  {#if icon}<Icon name={icon} size={16} {spin} />{/if}
  {@render children?.()}
  {#if iconAfter}<Icon name={iconAfter} size={16} />{/if}
</button>

<style>
  .btn {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 40px;
    padding: 0 18px;
    border-radius: 10px;
    border: 1px solid transparent;
    font: 600 14px/1 'Inter', sans-serif;
    white-space: nowrap;
    cursor: pointer;
    background: transparent;
    color: var(--text);
  }
  .small {
    height: 32px;
    padding: 0 12px;
    border-radius: 8px;
    font-size: 13px;
  }
  .primary { background: var(--accent); }
  .primary:hover { background: var(--accent-hover); }
  .secondary, .danger {
    background: var(--raised);
    border-color: var(--border);
  }
  .secondary:hover, .danger:hover { border-color: var(--accent); }
  .danger { color: var(--behind); }
  .ghost { color: var(--muted); }
  .ghost:hover { color: var(--text); }
  .btn.disabled,
  .btn.disabled:hover {
    background: var(--raised);
    border-color: transparent;
    color: var(--muted);
    cursor: default;
  }
</style>
