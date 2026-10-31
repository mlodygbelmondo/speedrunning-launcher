<script lang="ts" module>
  // Lucide SVGs (+ logo.svg) inlined so `currentColor` follows the text colour.
  const raw = import.meta.glob<string>('../../../assets/icons/*.svg', {
    eager: true,
    query: '?raw',
    import: 'default',
  });
  const icons: Record<string, string> = {};
  for (const [path, svg] of Object.entries(raw)) {
    const name = path.slice(path.lastIndexOf('/') + 1, -4);
    icons[name] = svg
      .replace(/<!--[\s\S]*?-->/g, '')
      .replace(/<svg[^>]*>/, (tag) => tag.replace(/\s(class|width|height)="[^"]*"/g, ''))
      .trim();
  }
</script>

<script lang="ts">
  let {
    name,
    size = 16,
    color,
    spin = false,
  }: { name: string; size?: number; color?: string; spin?: boolean } = $props();
</script>

<span
  class="icon"
  class:spin
  style:width="{size}px"
  style:height="{size}px"
  style:color
  aria-hidden="true">{@html icons[name] ?? ''}</span
>

<style>
  .icon {
    display: inline-block;
    flex: none;
    line-height: 0;
  }
  .icon :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
  /* Spinner: one turn per 900 ms, only rendered while something is working. */
  .spin {
    animation: spin 900ms linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
