<script lang="ts">
  import type { Snippet } from 'svelte';
  let {
    title,
    sub,
    actions,
    above,
  }: { title: string; sub: string; actions?: Snippet; above?: Snippet } = $props();
</script>

<header>
  {#if above}<div class="above">{@render above()}</div>{/if}
  <div class="row">
    <div class="titles">
      <h1>{title}</h1>
      <div class="sub">{sub}</div>
    </div>
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  </div>
</header>

<style>
  header { flex: none; }
  .above { margin: -6px 0 8px -12px; }
  .row {
    display: flex;
    align-items: flex-start;
    gap: 16px;
  }
  .titles {
    flex: 1 1 auto;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font: 700 26px/32px 'Inter', sans-serif;
  }
  .sub {
    margin-top: 4px;
    font-size: 13px;
    line-height: 18px;
    color: var(--muted);
  }
  /* Actions are vertically centred on the title line (32 px). */
  .actions {
    flex: none;
    height: 32px;
    display: flex;
    align-items: center;
    gap: 10px;
  }
</style>
