<script lang="ts">
  import { t } from '../copy.svelte';
  import { S } from '../screens';
  import { toast } from '../state.svelte';
  import Header from '../ui/Header.svelte';
  import Icon from '../ui/Icon.svelte';
  import Badge from '../ui/Badge.svelte';
  import Chip from '../ui/Chip.svelte';

  const LEVELS = ['beginner', 'intermediate', 'advanced'];
  const LEVEL_BADGE: Record<string, 'ok' | 'accent' | 'pb'> = { beginner: 'ok', intermediate: 'accent', advanced: 'pb' };

  let level = $state('');
  const groups = $derived(
    S.tutorial_groups
      .map((g) => ({ ...g, items: g.items.filter((it) => !level || it.level === level) }))
      .filter((g) => g.items.length),
  );
</script>

<Header title={t('learn.title')} sub={t('learn.subtitle')} />

<div class="chips">
  <Chip label={t('learn.filter_all')} on={!level} onclick={() => (level = '')} />
  {#each LEVELS as l (l)}
    <Chip label={t('learn.level_' + l)} on={level === l} onclick={() => (level = l)} />
  {/each}
</div>

<div class="scroll body">
  {#each groups as g (g.title)}
    <section>
      <h2 class="gtitle"><Icon name={g.icon} size={16} color="var(--accent)" />{g.title}</h2>
      <div class="grid">
        {#each g.items as it (it.url)}
          <button class="card tut" onclick={() => toast(t('common.opens_in_browser'))}>
            <span class="thumb">
              <span class="bigicon"><Icon name={g.icon} size={56} /></span>
              <span class="play"><Icon name={it.kind === 'guide' ? 'book-open' : 'play'} size={20} /></span>
              <span class="len mono">{it.length || t('learn.kind_' + it.kind)}</span>
            </span>
            <span class="text">
              <span class="title">{it.title}</span>
              <span class="creator">{it.creator}</span>
              <span class="meta">
                <Badge variant={LEVEL_BADGE[it.level]}>{t('learn.level_' + it.level)}</Badge>
                {#if it.current}<Badge variant="ok">{t('learn.current')}</Badge>{/if}
                <span class="grow"></span>
                <span class="date">{it.date}</span>
              </span>
            </span>
          </button>
        {/each}
      </div>
    </section>
  {/each}
</div>

<style>
  .chips {
    flex: none;
    display: flex;
    gap: 8px;
    margin-top: -4px;
  }
  .body {
    flex: 1 1 auto;
    min-height: 0;
    margin-top: -4px;
    padding-right: 8px;
    margin-right: -8px;
    scrollbar-gutter: stable;
  }
  section + section { margin-top: 28px; }
  .gtitle {
    margin: 0 0 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    font: 600 16px/22px 'Inter', sans-serif;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 16px;
  }
  .grow { flex: 1 1 auto; }
  .tut {
    padding: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    text-align: left;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .tut:hover { border-color: var(--accent); }
  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    background: linear-gradient(180deg, var(--raised), #120f1a);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .bigicon { color: rgb(139 92 246 / 0.35); line-height: 0; }
  .play {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 44px;
    height: 44px;
    margin: -22px 0 0 -22px;
    border-radius: 50%;
    background: rgb(0 0 0 / 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text);
  }
  .len {
    position: absolute;
    right: 8px;
    bottom: 8px;
    height: 22px;
    padding: 0 8px;
    border-radius: 6px;
    background: rgb(0 0 0 / 0.65);
    font-size: 11px;
    line-height: 22px;
    color: var(--text);
  }
  .text {
    padding: 12px 14px 14px;
    display: flex;
    flex-direction: column;
  }
  .title {
    height: 40px;
    font: 600 14px/20px 'Inter', sans-serif;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .creator {
    margin-top: 4px;
    font-size: 12px;
    line-height: 16px;
    color: var(--muted);
  }
  .meta {
    margin-top: 10px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .date { font-size: 12px; color: var(--muted); white-space: nowrap; }
</style>
