<script lang="ts">
  let { on = $bindable(false), label }: { on?: boolean; label?: string } = $props();
</script>

<button class="tg" class:on role="switch" aria-checked={on} aria-label={label} onclick={() => (on = !on)}
  ><span class="knob"></span></button
>

<style>
  .tg {
    flex: none;
    position: relative;
    width: 36px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 10px;
    background: var(--border);
    cursor: pointer;
  }
  .tg.on { background: var(--accent); }
  .knob {
    position: absolute;
    left: 2px;
    top: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text);
    transition: transform 120ms ease-out;
  }
  .on .knob { transform: translateX(16px); }
</style>
