<script lang="ts">
  // Scene v3 Practice (REDESIGN.md "Practice", mockup a-practice.html).
  import { t } from '../copy.svelte';
  import { S, type MapItem } from '../screens';
  import { ui, install, installState } from '../state.svelte';
  import { card, up } from '../art';

  const TOOLS = 'tools';
  // Thumbnail crops cycle so rows of one category don't repeat the same picture.
  const CROPS = ['50% 50%', '15% 60%', '85% 30%', '35% 80%', '70% 20%'];

  const cats = $derived(S.practice_categories);
  const art = (cat: string) => S.v3.practice_art[cat] ?? 'overworld';
  const sel = $derived(ui.practiceCat);
  const selArt = $derived(art(sel === TOOLS ? S.v3.practice_default : sel));
  const mapsOf = (cat: string) => S.maps.map((m, i) => ({ m, key: 'map:' + i })).filter(({ m }) => m.category === cat);
  const count = (n: number) => (n === 1 ? t('v3.practice_count_one') : t('v3.practice_count', { n }));
  const rows = $derived(
    sel === TOOLS
      ? S.practice_tools.map((m, i) => ({ m, key: 'ptool:' + i }))
      : mapsOf(sel),
  );
  const selName = $derived(sel === TOOLS ? t('v3.practice_tools_tile') : (cats.find((c) => c.id === sel)?.name ?? ''));
  const first = $derived(cats[0]);
  const firstMap = $derived(mapsOf(first.id)[0]?.m);
</script>

{#snippet tile(id: string, name: string, sub: string, big = false)}
  <button class="cat" class:big class:on={sel === id} onclick={() => (ui.practiceCat = id)}>
    <span class="img" style:background-image="url({card(art(id))})"></span>
    {#if sel !== id}<span class="dim"></span>{/if}
    <span class="grad"></span>
    <span class="tx"><span class="tt">{name}</span><span class="ts v-plex">{sub}</span></span>
  </button>
{/snippet}

{#snippet row(m: MapItem, key: string, i: number)}
  {@const st = installState(key, m.status)}
  <div class="r">
    <span class="th" style:background-image="url({card(sel === TOOLS ? 'overworld' : art(sel))})" style:background-position={CROPS[i % CROPS.length]}></span>
    <div class="nm"><b>{m.name}</b><span>{m.authors}</span></div>
    <div class="d">{m.desc}</div>
    <div class="v v-plex">{[m.version, m.downloads].filter(Boolean).join(' · ')}</div>
    <div class="a">
      {#if st === 'installed'}
        <span class="ok">{up(t('common.installed'))}</span>
      {:else if st === 'installing'}
        <span class="busy">{up(t('common.installing'))}</span>
      {:else if st === 'update'}
        <button class="act upd" onclick={() => install(key)}>{up(t('common.update'))}</button>
      {:else}
        <button class="act go" onclick={() => install(key)}>{up(t('common.install'))}</button>
      {/if}
    </div>
    {#if st === 'installing'}<span class="prog"><span class="fill"></span></span>{/if}
  </div>
{/snippet}

<div class="bg">
  <span class="img" style:background-image="url({card(selArt)})"></span>
  <span class="veil"></span>
  <span class="fade"></span>
</div>

<main>
  <div class="head">
    <h1>{t('practice.title')}</h1>
    <p>{t('practice.subtitle')}</p>
  </div>

  <div class="cats">
    {@render tile(first.id, first.name, firstMap ? `${firstMap.name} · ${firstMap.desc}` : count(0), true)}
    {#each cats.slice(1) as c (c.id)}
      {@render tile(c.id, c.name, count(mapsOf(c.id).length))}
    {/each}
    <button class="cat tools" class:on={sel === TOOLS} onclick={() => (ui.practiceCat = TOOLS)}>
      <span class="tx">
        <span class="tt">{t('v3.practice_tools_tile')}</span>
        <span class="ts v-plex">{S.practice_tools.map((x) => x.name).join(' · ')}</span>
      </span>
    </button>
  </div>

  <div class="lh">
    <span class="v-cap">{up(t('v3.practice_list', { category: selName, count: count(rows.length) }))}</span>
    <span class="v-cap">{up(t('v3.practice_sort'))}</span>
  </div>
  <div class="list scroll">
    {#each rows as { m, key }, i (key)}
      {@render row(m, key, i)}
    {/each}
  </div>
  <p class="tip">{t('practice.ranked_tip')}</p>
</main>

<style>
  .bg { position: absolute; left: 0; right: 0; top: 0; height: 360px; z-index: 0; overflow: hidden; }
  .bg .img { position: absolute; inset: 0; background: center 40% / cover no-repeat; }
  /* 22 % soft image = bg @ 78 % over the upscaled card (no blur filter, same as GPUI). */
  .veil { position: absolute; inset: 0; background: rgb(10 8 16 / 0.78); }
  .fade { position: absolute; inset: 0; background: linear-gradient(0deg, var(--bg), rgb(10 8 16 / 0)); }

  main {
    position: absolute;
    left: 64px;
    right: 48px;
    top: 104px;
    bottom: 24px;
    z-index: 1;
    display: flex;
    flex-direction: column;
    color: var(--v-text);
  }
  .head { flex: none; display: flex; align-items: flex-end; justify-content: space-between; gap: 32px; }
  h1 { margin: 0; font: 800 56px/0.9 var(--archivo); letter-spacing: -0.025em; white-space: nowrap; }
  .head p { margin: 0; max-width: 420px; font-size: 14px; line-height: 1.45; text-align: right; color: var(--v-muted); }

  .cats {
    flex: none;
    margin-top: 30px;
    display: grid;
    grid-template-columns: 1.5fr 1fr 1fr 1fr;
    grid-template-rows: 128px 128px;
    gap: 12px;
  }
  .cat {
    position: relative;
    min-width: 0;
    padding: 0;
    border: 0;
    background: #1a1424;
    color: var(--v-text);
    text-align: left;
    cursor: pointer;
  }
  .cat.big { grid-row: span 2; }
  .cat.on { outline: 2px solid var(--v-text); outline-offset: 3px; }
  .cat:not(.on):hover { outline: 1px solid var(--v-faint); outline-offset: 3px; }
  .cat .img, .dim, .grad { position: absolute; inset: 0; }
  .cat .img { background: center / cover no-repeat; }
  .dim { background: rgb(10 8 16 / 0.34); }
  .grad { background: linear-gradient(0deg, rgb(10 8 16 / 0.9), rgb(10 8 16 / 0) 65%); }
  .cat.tools { background: #17131f; box-shadow: inset 0 0 0 1px var(--v-line); }
  .tx { position: absolute; left: 16px; right: 16px; bottom: 14px; display: flex; flex-direction: column; }
  .tt { font: 700 20px/1 var(--archivo); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .big .tt { font-size: 30px; }
  .ts {
    margin-top: 6px;
    font-size: 11px;
    font-weight: 500;
    line-height: 1.3;
    color: var(--v-muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .lh { flex: none; margin-top: 30px; padding-bottom: 12px; display: flex; justify-content: space-between; gap: 24px; }
  .list { flex: 0 1 auto; min-height: 0; }
  .r {
    position: relative;
    height: 58px;
    display: flex;
    align-items: center;
    gap: 20px;
    border-top: 1px solid var(--v-line);
  }
  .th { flex: none; width: 84px; height: 46px; background: center / cover no-repeat; }
  .nm { flex: 0 0 250px; min-width: 0; }
  .nm b { display: block; font: 600 15px/1.2 'Inter', sans-serif; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .nm span { display: block; font-size: 12px; line-height: 1.35; color: var(--v-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .d { flex: 1 1 auto; min-width: 0; font-size: 13px; line-height: 1.35; color: var(--v-muted); }
  .v { flex: 0 0 130px; font-size: 12px; font-weight: 500; color: var(--v-faint); text-align: right; white-space: nowrap; }
  .a { flex: 0 0 150px; text-align: right; white-space: nowrap; font: 700 12px/1 var(--archivo); letter-spacing: 0.16em; }
  .a .ok { color: var(--v-ok); }
  .a .busy { color: var(--v-faint); }
  .act {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    letter-spacing: inherit;
    cursor: pointer;
  }
  .act.go { color: var(--v-accent-2); }
  .act.upd { color: var(--v-text); }
  .act:hover { text-decoration: underline; text-underline-offset: 4px; }
  /* Fake install progress, under the row: 1200 ms linear. */
  .prog { position: absolute; left: 0; right: 0; bottom: 0; height: 2px; background: var(--v-line); }
  .fill { display: block; height: 100%; background: var(--v-accent-2); transform-origin: left; animation: fill 1200ms linear forwards; }
  @keyframes fill {
    from { transform: scaleX(0); }
    to { transform: scaleX(1); }
  }
  .tip { flex: none; margin: 10px 0 0; font-size: 12px; line-height: 1.4; color: var(--v-faint); }
</style>
