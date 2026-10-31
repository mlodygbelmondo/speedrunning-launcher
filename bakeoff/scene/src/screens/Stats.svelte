<script lang="ts">
  import { t } from '../copy.svelte';
  import { S } from '../screens';
  import { go } from '../state.svelte';
  import Header from '../ui/Header.svelte';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import Segmented from '../ui/Segmented.svelte';

  const st = $derived(S.stats);
  const tl = $derived(st.tiles);
  const tiles = $derived([
    { label: 'stats.tile_pb', value: tl.pb, sub: tl.pb_category, pb: true },
    { label: 'stats.tile_runs', value: tl.runs_month, sub: tl.runs_month_sub, pb: false },
    { label: 'stats.tile_finish', value: tl.finished, sub: tl.finished_sub, pb: false },
    { label: 'stats.tile_avg_enter', value: tl.avg_enter, sub: tl.avg_enter_sub, pb: false },
  ]);
</script>

<Header title={t('stats.title')} sub={t('stats.subtitle', { n: st.total_runs })}>
  {#snippet actions()}
    <Segmented
      items={[t('stats.tab_overview'), t('stats.tab_leaderboard')]}
      value={0}
      onchange={(i) => i === 1 && go('library')}
    />
  {/snippet}
</Header>

<div class="tiles">
  {#each tiles as tile (tile.label)}
    <div class="card tile">
      <div class="label">{t(tile.label)}</div>
      <div class="value mono" class:pbc={tile.pb}>{tile.value}</div>
      <div class="sub" class:ahead={tile.sub.startsWith('-')}>{tile.sub}</div>
    </div>
  {/each}
</div>

<div class="card recent">
  <h2 class="card-title bullet">{t('stats.recent')}</h2>
  <div class="table">
    <div class="tr head">
      <span class="c-date">{t('stats.col_date')}</span>
      <span class="c-mode">{t('stats.col_mode')}</span>
      <span class="c-res">{t('stats.col_result')}</span>
      {#each st.splits as s (s)}<span class="c-split">{s}</span>{/each}
      <span class="c-act"></span>
    </div>
    {#each st.runs as r, i (i)}
      <div class="tr">
        <span class="c-date muted">{r.date}</span>
        <span class="c-mode">{r.mode}</span>
        <span class="c-res">
          {#if r.finished}
            <span class="mono time" class:pbc={r.pb}>{r.result}</span>
            {#if r.pb}<Badge variant="pb">{t('stats.pb')}</Badge>{/if}
          {:else}
            <span class="muted reset">{t('stats.reset_at', { split: r.result })}</span>
          {/if}
        </span>
        {#each st.splits as _s, j (j)}
          {@const sp = r.splits[j]}
          <span class="c-split">
            {#if sp}
              <span class="mono stime">{sp[0]}</span>
              {#if sp[1]}<span class="mono sdelta" class:ahead={sp[2]} class:behind={!sp[2]}>{sp[1]}</span>{/if}
            {:else}
              <span class="none">—</span>
            {/if}
          </span>
        {/each}
        <span class="c-act">
          {#if r.finished && r.mode === 'RSG'}
            <Button small onclick={() => go('submit')}>{t('stats.submit')}</Button>
          {/if}
        </span>
      </div>
    {/each}
  </div>
</div>

<style>
  .tiles {
    flex: none;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 16px;
  }
  .tile { height: 112px; }
  .label { font-size: 13px; line-height: 18px; color: var(--muted); }
  .value {
    margin-top: 4px;
    font-weight: 700;
    font-size: 28px;
    line-height: 36px;
  }
  .sub { font-size: 12px; line-height: 16px; color: var(--muted); }
  .sub.ahead { color: var(--ahead); }

  .recent {
    flex: 1 1 auto;
    min-height: 0;
    margin-top: -4px; /* 16 px below the tiles */
    padding-bottom: 6px;
  }
  /* 45 px rows: the v3 top nav starts the page at 104 px, so 8 runs still fit at 1280x800. */
  .tr {
    height: 45px;
    display: flex;
    align-items: center;
  }
  .tr + .tr { border-top: 1px solid var(--border); }
  .tr.head {
    height: 32px;
    font: 500 12px/16px 'Inter', sans-serif;
    color: var(--muted);
  }
  .c-date { flex: 0 0 80px; font-size: 13px; }
  .c-mode { flex: 0 0 90px; font-size: 13px; }
  .head .c-date, .head .c-mode { font-size: 12px; }
  .c-res {
    flex: 0 0 170px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .time { font-size: 14px; }
  .reset { font-size: 13px; }
  .c-split {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
  }
  .stime { font-size: 13px; line-height: 17px; }
  .sdelta { font-size: 11px; line-height: 14px; }
  .none { color: var(--border); font-size: 13px; }
  .c-act {
    flex: 0 0 90px;
    display: flex;
    justify-content: flex-end;
  }
</style>
