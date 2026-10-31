<script lang="ts">
  // Scene v3 Modes (REDESIGN.md "Modes", mockup a-modes.html).
  import { t } from '../copy.svelte';
  import { S, type Mode } from '../screens';
  import { ui, installState } from '../state.svelte';
  import { hero, card, up } from '../art';
  import PlayButton from '../ui/PlayButton.svelte';

  const STATUS_COLOR: Record<string, string> = { ready: 'var(--v-ok)', update: 'var(--v-gold)', missing: 'var(--v-accent-2)' };

  const sel = $derived((S.modes.find((m) => m.id === ui.mode) ?? S.modes[0]) as Mode);
  const v = $derived(S.v3.modes[sel.id]);
  const status = (m: Mode) => {
    const s = installState('mode:' + m.id, ui.modeStatus[m.id]);
    return s === 'installing' ? 'missing' : s;
  };
  const others = (lid: string, m: Mode) => S.modes.filter((o) => o.id !== m.id && o.layers.includes(lid));
  function layerDetail(lid: string): string {
    const l = S.layers[lid];
    if (!l.mods.length) return l.detail ?? '';
    if (l.mods.length === 1) return `${t('modes.mods_count_one')} · ${l.mods[0][1]}`;
    return t('modes.mods_count', { n: l.mods.length });
  }
</script>

<div class="bg">
  <img class="art" src={hero(v.art)} alt="" draggable="false" />
  <span class="veil"></span>
  <span class="grad"></span>
</div>

<section class="list">
  {#each S.modes as m (m.id)}
    {@const st = status(m)}
    <button class="mr" class:on={m.id === sel.id} onclick={() => (ui.mode = m.id)}>
      <span class="img" style:background-image="url({card(S.v3.modes[m.id].art)})"></span>
      <span class="shade"></span>
      <span class="mt">
        <b>{S.v3.modes[m.id].short}</b>
        <span class="v-plex">{m.full}</span>
      </span>
      <span class="ms" style:color={STATUS_COLOR[st]}>
        {up(t('v3.status.' + st))}
        <em class="v-plex">{m.mc}</em>
      </span>
    </button>
  {/each}
</section>

<section class="det">
  <div class="v-eyebrow">{up(t('v3.eyebrow.' + sel.id, { season: S.v3.season, mc: sel.mc }))}</div>
  <h2>{v.short}</h2>
  <p class="desc">{t('v3.desc.' + sel.id)}</p>
  <div class="facts">
    <div><span class="v-cap">{up(t('v3.f_mc'))}</span>{sel.mc}</div>
    <div><span class="v-cap">{up(t('v3.f_mods'))}</span>{v.mods_total}</div>
    <div><span class="v-cap">{up(t('v3.f_disk'))}</span>{v.disk}</div>
    {#if sel.leaderboard}<div><span class="v-cap">{up(t('v3.f_board'))}</span>{sel.leaderboard}</div>{/if}
  </div>
  <div class="lh">
    <span class="v-cap">{up(t('v3.layers'))}</span>
    <span class="v-cap">{up(t('v3.layers_saved', { size: S.disk_saved }))}</span>
  </div>
  <div class="stack">
    {#each sel.layers as lid (lid)}
      {@const sh = others(lid, sel)}
      <div class="ly" class:own={!sh.length}>
        <b>{S.layers[lid].name}</b>
        <span class="v-plex">{layerDetail(lid)}</span>
        <em class="v-plex">
          {sh.length ? t('v3.shared_with', { list: sh.map((o) => S.v3.modes[o.id].short).join(', ') }) : t('v3.only_this')}
        </em>
      </div>
    {/each}
  </div>
  <div class="acts">
    <PlayButton mode={sel.id} small />
    <!-- A mode that isn't installed (or is still installing) has nothing to verify or open. -->
    {#if status(sel) !== 'missing'}
      <button class="v-ghost">{up(t('v3.verify'))}</button>
      <button class="v-ghost">{up(t('v3.open_folder'))}</button>
    {/if}
  </div>
  <p class="note">{t('v3.note.' + sel.id)}</p>
</section>

<style>
  .bg { position: absolute; inset: 0; z-index: 0; background: var(--bg); overflow: hidden; }
  .art { position: absolute; left: 0; top: 0; width: 1280px; height: 800px; }
  /* 16 % image = bg @ 84 % over it (no opacity on the image, same as GPUI). */
  .veil { position: absolute; inset: 0; background: rgb(10 8 16 / 0.84); }
  .grad {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, var(--bg) 0%, rgb(10 8 16 / 0.7) 50%, rgb(10 8 16 / 0.85) 100%),
      linear-gradient(0deg, var(--bg), rgb(10 8 16 / 0) 60%);
  }

  .list, .det { position: absolute; z-index: 2; top: 104px; color: var(--v-text); }
  .list {
    left: 64px;
    width: 432px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .mr {
    position: relative;
    height: 104px;
    padding: 0;
    border: 0;
    background: #1a1424;
    color: var(--v-text);
    text-align: left;
    cursor: pointer;
  }
  .mr.on { outline: 2px solid var(--v-text); outline-offset: 3px; }
  .mr:not(.on):hover { outline: 1px solid var(--v-faint); outline-offset: 3px; }
  .img, .shade { position: absolute; inset: 0; }
  .img { background: center / cover no-repeat; }
  .shade { background: linear-gradient(90deg, rgb(10 8 16 / 0.95) 0%, rgb(10 8 16 / 0.78) 60%, rgb(10 8 16 / 0.6) 100%); }
  .on .shade { background: linear-gradient(90deg, rgb(10 8 16 / 0.92) 0%, rgb(10 8 16 / 0.55) 55%, rgb(10 8 16 / 0.25) 100%); }
  .mt { position: absolute; left: 20px; top: 26px; display: flex; flex-direction: column; }
  .mt b { font: 800 30px/1 var(--archivo); letter-spacing: -0.02em; white-space: nowrap; }
  .mt span { margin-top: 8px; font-size: 11px; font-weight: 500; line-height: 1; color: var(--v-muted); white-space: nowrap; }
  .ms {
    position: absolute;
    right: 18px;
    top: 18px;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font: 600 10px/1 var(--archivo);
    letter-spacing: 0.16em;
    white-space: nowrap;
  }
  .ms em { margin-top: 8px; font-size: 11px; font-weight: 500; font-style: normal; letter-spacing: 0; color: var(--v-faint); }

  .det { left: 560px; right: 48px; }
  .det .v-eyebrow { margin-top: 4px; }
  h2 {
    margin: 12px 0 0;
    font: 800 72px/0.9 var(--archivo);
    letter-spacing: -0.03em;
    white-space: nowrap;
  }
  .desc {
    margin: 14px 0 0;
    max-width: 600px;
    font-size: 15px;
    line-height: 1.45;
    color: var(--v-muted);
  }
  .facts { margin-top: 22px; display: flex; gap: 36px; }
  .facts div { font: 500 15px/1 var(--plex); white-space: nowrap; }
  .facts .v-cap { display: block; margin-bottom: 8px; }
  .lh { margin-top: 30px; display: flex; justify-content: space-between; gap: 24px; }
  .stack { margin-top: 12px; display: flex; flex-direction: column; gap: 3px; }
  .ly {
    height: 40px;
    padding: 0 16px;
    display: flex;
    align-items: center;
    gap: 14px;
    background: rgb(241 238 248 / 0.05);
    white-space: nowrap;
  }
  .ly b { flex: 0 0 210px; font: 600 14px/1 'Inter', sans-serif; overflow: hidden; text-overflow: ellipsis; }
  .ly span { flex: 1 1 auto; min-width: 0; font-size: 12px; line-height: 1; color: var(--v-muted); overflow: hidden; text-overflow: ellipsis; }
  .ly em { flex: none; font-size: 11px; font-weight: 500; font-style: normal; line-height: 1; color: var(--v-faint); }
  .ly.own { background: rgb(139 92 246 / 0.22); box-shadow: inset 3px 0 0 var(--v-accent-2); }
  .ly.own em { color: var(--v-accent-2); }
  .acts { margin-top: 26px; display: flex; align-items: center; gap: 28px; }
  .note {
    margin: 18px 0 0;
    max-width: 640px;
    font-size: 12px;
    line-height: 1.45;
    color: var(--v-faint);
  }
</style>
