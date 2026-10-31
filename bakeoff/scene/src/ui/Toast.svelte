<script lang="ts">
  import { ui } from '../state.svelte';
</script>

{#if ui.toast}
  {#key ui.toast.id}
    <div class="toast" role="status">{ui.toast.msg}</div>
  {/key}
{/if}

<style>
  /* 150 ms in, 1800 ms shown, 150 ms out = 2100 ms; it stops by itself. */
  .toast {
    position: fixed;
    left: 50%;
    bottom: 24px;
    z-index: 50;
    transform: translateX(-50%);
    padding: 10px 16px;
    border-radius: 12px;
    border: 1px solid var(--border);
    background: var(--raised);
    font: 500 13px/18px 'Inter', sans-serif;
    white-space: nowrap;
    pointer-events: none;
    animation: toast 2100ms linear forwards;
  }
  @keyframes toast {
    0% { opacity: 0; }
    7.143% { opacity: 1; }
    92.857% { opacity: 1; }
    100% { opacity: 0; }
  }
</style>
