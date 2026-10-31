<script lang="ts">
  // Scene v3 Home (REDESIGN.md "Home"): one layout per selected mode, over full-window key art.
  import { t } from './copy.svelte';
  import { S } from './screens';
  import { avatar } from './data';
  import { ui, installState } from './state.svelte';
  import { hero, card, shade, up, TIER_COLOR } from './art';
  import PlayButton from './ui/PlayButton.svelte';

  const STATUS_COLOR: Record<string, string> = { ready: 'var(--v-ok)', update: 'var(--v-gold)', missing: 'var(--v-accent-2)' };
  // The seed mod shown for run modes, and the tools that start with them (names come from the data).
  const KEY_MOD: Record<string, [string, string]> = { rsg: ['rsg', 'SeedQueue'], fsg: ['fsg', 'FSG Mod'] };
  const RUN_TOOLS = ['ninjabrain', 'paceman'];

  const id = $derived(ui.mode);
  const m = $derived(S.modes.find((x) => x.id === id) ?? S.modes[0]);
  const v = $derived(S.v3.modes[id]);
  const st = $derived(installState('mode:' + id, ui.modeStatus[id]));
  const missing = $derived(st === 'missing' || st === 'installing');
  const r = $derived(S.v3.ranked);
  const run = $derived(S.v3.runs[id]);
  const install = $derived(S.v3.install[id]);

  const extra = $derived.by(() => {
    if (missing) return t('v3.download_hint', { size: v.download ?? v.disk });
    if (id === 'ranked') return t('v3.online', { n: S.v3.online });
    const km = KEY_MOD[id];
    const mod = km && S.layers[km[0]].mods.find(([n]) => n === km[1]);
    const tools = RUN_TOOLS.map((tid) => S.tools.find((x) => x.id === tid)?.name.split(' ')[0]).filter(Boolean);
    return [mod ? (/^\d/.test(mod[1]) ? `${mod[0]} ${mod[1]}` : mod[0]) : '', ...tools].filter(Boolean).join(' · ');
  });
  const statusLine = $derived(missing ? m.mc : `${m.mc} · ${t('v3.mods_checked', { n: v.mods_total })}`);
  const eyebrow = $derived(t('v3.eyebrow.' + id, { season: S.v3.season, mc: m.mc }));
  const delta = (s: string) => s.replace(/^-/, '−');
  const arrow = (s: string) => (s.startsWith('-') ? '▼ ' : '▲ ') + s.replace(/^[+-]/, '');
</script>

<div class="bg">
  <img class="art" src={hero(v.art)} alt="" draggable="false" />
  <img class="art" src={shade} alt="" draggable="false" />
</div>

<section class="hero">
  <div class="v-eyebrow">{up(eyebrow)}</div>
  <h1>{v.short}</h1>
  <p class="desc">{t('v3.desc.' + id)}</p>
  <div class="status v-plex">
    <span class="st" style:color={STATUS_COLOR[missing ? 'missing' : st]}
      ><i style:background={STATUS_COLOR[missing ? 'missing' : st]}></i>{t('v3.status.' + (missing ? 'missing' : st))}</span
    >
    <span>{statusLine}</span>
    <span>{extra}</span>
  </div>
  <div class="actions">
    <PlayButton mode={id} />
    {#if !missing}<button class="v-ghost">{up(t('v3.verify'))}</button>{/if}
  </div>
</section>

{#if id === 'ranked'}
  <aside class="form">
    <div>
      <div class="v v-plex">{r.elo}<span class="up">{arrow(r.elo_delta)}</span></div>
      <div class="l"><i style:background={TIER_COLOR[r.tier]}></i>{up(t('v3.r.elo', { tier: t('tier.' + r.tier) }))}</div>
    </div>
    <div>
      <div class="v v-plex">{r.season_w} – {r.season_l}</div>
      <div class="l">{up(t('v3.r.season', { season: S.v3.season }))}</div>
    </div>
    <div>
      <div class="v v-plex">{r.best}</div>
      <div class="l">{up(t('v3.r.best'))}</div>
    </div>
  </aside>

  <section class="band">
    <div class="h"><span class="v-cap">{up(t('v3.matches'))}</span><span class="v-cap">{up(t('v3.matches_src'))}</span></div>
    <div class="mrow">
      {#each r.matches as mt, i (i)}
        <div class="m" class:lost={!mt.won}>
          <img src={avatar(mt.avatar)} alt="" width="26" height="26" />
          <div class="who">
            {mt.opponent}
            <span class="v-plex">{mt.won ? t('v3.match_won', { time: mt.time }) : t('v3.match_lost', { time: mt.time })}</span>
          </div>
          <div class="e v-plex">{delta(mt.elo)}</div>
        </div>
      {/each}
      <div class="m weekly">
        <div class="who">
          {t('v3.weekly', { n: r.weekly.n })}
          <span class="v-plex">{t('v3.weekly_place', { place: r.weekly.place, total: r.weekly.total })}</span>
        </div>
        <div class="e v-plex">{r.weekly.time}</div>
      </div>
    </div>
  </section>
{:else if run}
  <aside class="form">
    <div>
      <div class="v v-plex">{run.pb}</div>
      <div class="l">{up(t('v3.pb', { date: run.pb_date }))}</div>
    </div>
    <div>
      <div class="v v-plex gold">{run.sob}</div>
      <div class="l">{up(t('v3.sob'))}</div>
    </div>
    <div>
      <div class="v v-plex">{run.runs_month}</div>
      <div class="l">{up(t('v3.runs_month'))}</div>
    </div>
  </aside>

  <section class="band">
    <div class="h"><span class="v-cap">{up(t('v3.strip'))}</span><span class="v-cap">{up(t('v3.strip_src'))}</span></div>
    <div class="segs">
      {#each run.splits as sp (sp.name)}
        <div class="seg" style:flex="{sp.secs} 1 0" style:background-image="url({card(sp.art)})" title="{sp.name} {sp.time}">
          <div>
            <span class="t">{up(sp.name)}</span>
            <span class="d v-plex">{sp.time}<em>{sp.best}</em></span>
          </div>
        </div>
      {/each}
    </div>
  </section>
{:else if install}
  <section class="band">
    <div class="h"><span class="v-cap">{up(t('v3.install_title'))}</span></div>
    <div class="chips">
      {#each install.new as lid (lid)}
        <div class="chip"><span class="cn">{S.layers[lid].name}</span><span class="tag new">{up(t('v3.install_new'))}</span></div>
      {/each}
      {#each install.have as lid (lid)}
        <div class="chip"><span class="cn">{S.layers[lid].name}</span><span class="tag">{up(t('v3.install_have'))}</span></div>
      {/each}
    </div>
  </section>
{/if}

<section class="strip">
  <div class="h">
    <span class="v-cap">{up(t('v3.modes'))}</span>
    <span class="v-cap">{up(t('v3.modes_shared', { size: S.disk_saved }))}</span>
  </div>
  <div class="row">
    {#each S.modes as mo (mo.id)}
      {@const ms = installState('mode:' + mo.id, ui.modeStatus[mo.id])}
      <button class="tile" class:on={mo.id === id} onclick={() => (ui.mode = mo.id)}>
        <span class="img" style:background-image="url({card(S.v3.modes[mo.id].art)})"></span>
        {#if mo.id !== id}<span class="dim"></span>{/if}
        <span class="grad"></span>
        {#if ms === 'missing' || ms === 'installing'}<span class="badge">{up(t('v3.badge_download'))}</span>{/if}
        <span class="tx">
          <span class="tt">{S.v3.modes[mo.id].short}</span>
          <span class="ts v-plex">{mo.mc}</span>
        </span>
      </button>
    {/each}
  </div>
</section>

<style>
  .bg {
    position: absolute;
    inset: 0;
    z-index: 0;
    background: var(--bg);
  }
  .art {
    position: absolute;
    left: 0;
    top: 0;
    width: 1280px;
    height: 800px;
    pointer-events: none;
  }
  .hero, .form, .band, .strip { position: absolute; z-index: 2; color: var(--v-text); }

  .hero { left: 64px; top: 116px; width: 560px; }
  h1 {
    margin: 16px 0 0;
    font: 800 112px/0.86 var(--archivo);
    letter-spacing: -0.03em;
    white-space: nowrap;
  }
  .desc {
    margin: 20px 0 0;
    max-width: 440px;
    font-size: 16px;
    line-height: 1.5;
    color: var(--v-muted);
  }
  .status {
    margin-top: 16px;
    display: flex;
    gap: 18px;
    font-size: 12px;
    font-weight: 500;
    line-height: 1;
    color: var(--v-faint);
    white-space: nowrap;
  }
  .status i {
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-right: 8px;
    vertical-align: 1px;
  }
  .actions {
    margin-top: 30px;
    display: flex;
    align-items: center;
    gap: 28px;
  }

  .form {
    right: 48px;
    top: 116px;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 22px;
    text-align: right;
  }
  .form .v {
    font-weight: 600;
    font-size: 26px;
    line-height: 1;
    letter-spacing: -0.01em;
    white-space: nowrap;
  }
  .form .v.gold { color: var(--v-gold); }
  .form .up { margin-left: 6px; font-size: 13px; color: var(--v-ahead); }
  .form .l {
    margin-top: 7px;
    font: 500 11px/1 var(--archivo);
    letter-spacing: 0.16em;
    color: var(--v-muted);
    white-space: nowrap;
  }
  .form .l i {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 7px;
    vertical-align: 1px;
  }

  .band { left: 64px; right: 48px; top: 494px; }
  .h {
    display: flex;
    justify-content: space-between;
    gap: 24px;
    margin-bottom: 12px;
  }
  .mrow { display: flex; gap: 14px; }
  .m {
    flex: 1 1 0;
    min-width: 0;
    height: 52px;
    /* 12/10 instead of the mockup's 14/12: the Polish "Twoje miejsce: 214 / 3 980" and "rywal 13:20.551" fit. */
    padding: 0 12px;
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgb(241 238 248 / 0.04);
    border-left: 3px solid var(--v-ahead);
  }
  .m.lost { border-left-color: var(--v-behind); }
  .m.weekly { border-left-color: transparent; }
  .m img { width: 26px; height: 26px; image-rendering: pixelated; }
  .who {
    flex: 1 1 auto;
    min-width: 0;
    font: 600 13px/1.2 'Inter', sans-serif;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .who span {
    display: block;
    font-size: 11px;
    font-weight: 400;
    line-height: 1.3;
    color: var(--v-muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .e { font-size: 13px; font-weight: 500; color: var(--v-ahead); }
  .m.lost .e { color: var(--v-behind); }
  .m.weekly .e { color: var(--v-muted); }

  .segs { display: flex; gap: 3px; height: 66px; }
  .seg {
    position: relative;
    min-width: 0;
    overflow: hidden;
    background: #1a1622 center / cover no-repeat;
  }
  .seg::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgb(10 8 16 / 0.92) 0%, rgb(10 8 16 / 0.25) 80%);
  }
  .seg > div {
    position: absolute;
    left: 10px;
    right: 8px;
    bottom: 9px;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 5px;
    white-space: nowrap;
  }
  .seg .t { font: 600 11px/1 var(--archivo); letter-spacing: 0.14em; }
  .seg .d { font-size: 13px; font-weight: 500; line-height: 1; }
  .seg .d em { margin-left: 6px; font-style: normal; font-size: 11px; color: var(--v-gold); }

  .chips { display: flex; gap: 3px; }
  .chip {
    height: 40px;
    padding: 0 16px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
    background: rgb(241 238 248 / 0.05);
    white-space: nowrap;
  }
  .cn { font: 600 13px/1 'Inter', sans-serif; }
  .tag { font: 600 9px/1 var(--archivo); letter-spacing: 0.16em; color: var(--v-faint); }
  .tag.new { color: var(--v-accent-2); }

  .strip { left: 64px; right: 48px; bottom: 36px; }
  .strip .h { margin-bottom: 14px; }
  .row { display: flex; gap: 14px; }
  .tile {
    position: relative;
    flex: 1 1 0;
    min-width: 0;
    height: 116px;
    padding: 0;
    border: 0;
    background: #1a1424;
    color: var(--v-text);
    text-align: left;
    cursor: pointer;
  }
  .tile.on { outline: 2px solid var(--v-text); outline-offset: 3px; }
  .tile:not(.on):hover { outline: 1px solid var(--v-faint); outline-offset: 3px; }
  .img, .dim, .grad { position: absolute; inset: 0; }
  .img { background: center / cover no-repeat; }
  .dim { background: rgb(10 8 16 / 0.5); }
  .grad { background: linear-gradient(0deg, rgb(10 8 16 / 0.9), rgb(10 8 16 / 0) 70%); }
  .badge {
    position: absolute;
    right: 10px;
    top: 10px;
    padding: 5px 7px;
    background: rgb(10 8 16 / 0.75);
    font: 600 10px/1 var(--archivo);
    letter-spacing: 0.14em;
    color: var(--v-accent-2);
  }
  .tx { position: absolute; left: 14px; bottom: 12px; display: flex; flex-direction: column; }
  .tt { font: 700 17px/1 var(--archivo); }
  .ts { margin-top: 6px; font-size: 11px; font-weight: 500; line-height: 1; color: var(--v-muted); }
</style>
