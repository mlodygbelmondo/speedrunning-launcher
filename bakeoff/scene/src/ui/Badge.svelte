<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let {
    variant = 'muted',
    icon,
    mono = false,
    children,
  }: { variant?: 'ok' | 'warn' | 'muted' | 'pb' | 'accent'; icon?: string; mono?: boolean; children?: Snippet } =
    $props();
</script>

<span class="badge {variant}" class:mono>
  {#if icon}<Icon name={icon} size={12} />{/if}
  {@render children?.()}
</span>

<style>
  .badge {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 8px;
    border-radius: 6px;
    font: 600 11px/1 'Inter', sans-serif;
    white-space: nowrap;
  }
  .mono { font-family: var(--mono); font-weight: 400; }
  .ok { color: var(--ahead); background: rgb(74 222 128 / 0.12); }
  .warn { color: var(--warn); background: rgb(251 191 36 / 0.12); }
  .muted { color: var(--muted); background: var(--raised); }
  .pb { color: var(--pb); background: rgb(192 132 252 / 0.14); }
  .accent { color: var(--text); background: rgb(139 92 246 / 0.3); }
</style>
