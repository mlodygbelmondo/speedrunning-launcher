<script lang="ts">
  import Icon from './Icon.svelte';
  let { status, size = 20 }: { status: 'ok' | 'warn' | 'info' | 'error'; size?: number } = $props();
  const map = {
    ok: ['circle-check', 'var(--ahead)'],
    warn: ['triangle-alert', 'var(--warn)'],
    info: ['info', 'var(--muted)'],
    error: ['circle-x', 'var(--behind)'],
  } as const;
</script>

<Icon name={map[status][0]} {size} color={map[status][1]} />
