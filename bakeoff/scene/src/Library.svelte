<script lang="ts">
  import { data, avatar } from './data';
  import { t } from './copy.svelte';
  import { go } from './state.svelte';
  import Header from './ui/Header.svelte';
  import Segmented from './ui/Segmented.svelte';

  let { scroller = $bindable() }: { scroller?: HTMLElement } = $props();
</script>

<Header title={t('library.title')} sub={t('library.subtitle')}>
  {#snippet actions()}
    <Segmented
      items={[t('stats.tab_overview'), t('stats.tab_leaderboard')]}
      value={1}
      onchange={(i) => i === 0 && go('stats')}
    />
  {/snippet}
</Header>

<div class="list" bind:this={scroller}>
  {#each data.rows as r (r.rank)}
    <div class="row">
      <div class="rank mono">{r.rank}</div>
      <img class="av" src={avatar(r.avatar)} alt="" width="48" height="48" loading="lazy" decoding="async" />
      <div class="who">
        <div class="name">{r.name}</div>
        <div class="cat">{r.category}</div>
      </div>
      <div class="time mono">{r.time}</div>
      <div class="delta mono" class:ahead={r.ahead} class:behind={!r.ahead}>{r.delta}</div>
      <div class="elo mono">{r.elo}</div>
    </div>
  {/each}
</div>

<style>
  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    scrollbar-width: thin;
    scrollbar-color: var(--border) transparent;
  }
  .row {
    height: 72px; /* border-box: includes the 1px separator */
    padding: 0 18px;
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .row + .row {
    border-top: 1px solid var(--border);
  }
  .row:hover {
    background: var(--raised);
  }
  .rank {
    flex: 0 0 44px;
    width: 44px;
    font-size: 13px;
    color: var(--muted);
  }
  .av {
    flex: 0 0 48px;
    width: 48px;
    height: 48px;
    border-radius: 8px;
    image-rendering: pixelated;
  }
  .who {
    flex: 1 1 auto;
    min-width: 0;
  }
  .name {
    font-weight: 600;
    font-size: 15px;
    line-height: 20px;
  }
  .cat {
    font-size: 12px;
    line-height: 16px;
    color: var(--muted);
    margin-top: 2px;
  }
  .time {
    flex: 0 0 110px;
    text-align: right;
    font-size: 15px;
  }
  .delta {
    flex: 0 0 70px;
    text-align: right;
    font-size: 13px;
  }
  .elo {
    flex: 0 0 56px;
    text-align: right;
    font-size: 13px;
    color: var(--pb);
  }
</style>
