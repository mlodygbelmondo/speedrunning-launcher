<script lang="ts">
  // Scene v3 Play / Download button (REDESIGN.md "Home → Actions"). Pixel corners = two overlapping rects.
  import { t } from '../copy.svelte';
  import { S } from '../screens';
  import { ui, install, installState, openCheck } from '../state.svelte';
  import { up } from '../art';

  let { mode, small = false }: { mode: string; small?: boolean } = $props();

  const v = $derived(S.v3.modes[mode]);
  const st = $derived(installState('mode:' + mode, ui.modeStatus[mode]));
  const label = $derived(st === 'installing' ? t('common.installing') : st === 'missing' ? t('v3.download') : t('v3.play'));
  const hint = $derived(
    st === 'missing' || st === 'installing'
      ? t('v3.download_hint', { size: v.download ?? v.disk })
      : st === 'update'
        ? t('v3.play_hint.update', { mod: v.update_mod ?? '' })
        : mode === 'ranked'
          ? t('v3.play_hint.ranked')
          : t('v3.play_hint.run'),
  );

  function click() {
    if (st === 'installing') return;
    if (st === 'missing') install('mode:' + mode, () => (ui.modeStatus[mode] = 'ready'));
    else openCheck(mode);
  }
</script>

<button class="play" class:small onclick={click}>
  <span class="px h"></span>
  <span class="px v"></span>
  <span class="label">{up(label)}</span>
  <span class="hint">{hint}</span>
  {#if st === 'installing'}<span class="prog"><span class="fill"></span></span>{/if}
</button>

<style>
  .play {
    position: relative;
    flex: none;
    width: 288px;
    height: 64px;
    padding: 0;
    border: 0;
    background: none;
    color: #120d1c;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  .play.small { width: 280px; height: 58px; }
  .px { position: absolute; background: var(--v-text); }
  .px.h { left: 4px; right: 4px; top: 0; bottom: 0; }
  .px.v { left: 0; right: 0; top: 4px; bottom: 4px; }
  .play:hover .px { background: #fff; }
  .label {
    position: relative;
    font: 800 19px/1 var(--archivo);
    letter-spacing: 0.2em;
    margin-right: -0.2em; /* centre the tracked word */
  }
  .small .label { font-size: 17px; }
  .hint {
    position: relative;
    margin-top: 7px;
    font: 500 10px/1 var(--plex);
    letter-spacing: 0.04em;
    color: rgb(18 13 28 / 0.55);
    white-space: nowrap;
  }
  /* Fake install progress, under the button: 1200 ms linear, then it is removed. */
  .prog {
    position: absolute;
    left: 0;
    right: 0;
    bottom: -10px;
    height: 3px;
    background: var(--v-line);
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--v-accent-2);
    transform-origin: left;
    animation: fill 1200ms linear forwards;
  }
  @keyframes fill {
    from { transform: scaleX(0); }
    to { transform: scaleX(1); }
  }
</style>
