<script lang="ts">
  // Scene v3 shell: top nav bar (REDESIGN.md "Shell"). Drawn over the page, no background of its own.
  import { t } from '../copy.svelte';
  import { S } from '../screens';
  import { avatar } from '../data';
  import { activeAccount, type Page } from '../state.svelte';
  import { up, TIER_COLOR } from '../art';
  import Icon from './Icon.svelte';

  let { shown, onnav }: { shown: Page; onnav: (p: Page) => void } = $props();

  const items: [Page, string][] = [
    ['home', 'nav.play'],
    ['modes', 'nav.modes'],
    ['practice', 'nav.practice'],
    ['learn', 'nav.learn'],
    ['tools', 'nav.tools'],
    ['stats', 'nav.stats'],
  ];
  const active = $derived<Page>(shown === 'library' || shown === 'submit' ? 'stats' : shown);
  const r = $derived(S.v3.ranked);
</script>

<header class="top">
  <div class="brand">
    <span class="logo"><Icon name="logo" size={22} /></span>
    <span>SPEEDRUNLUNCHER</span>
  </div>
  <nav>
    {#each items as [page, key] (page)}
      <button class:on={active === page} onclick={() => onnav(page)}>{up(t(key))}</button>
    {/each}
  </nav>
  <div class="me">
    <button class="gear" class:on={shown === 'settings'} title={t('nav.settings')} aria-label={t('nav.settings')} onclick={() => onnav('settings')}>
      <Icon name="settings" size={18} />
    </button>
    <button class="acct" title={t('rail.account')} onclick={() => onnav('accounts')}>
      <span class="who">
        <span class="n">{activeAccount().name}</span>
        <span class="e"><i style:background={TIER_COLOR[r.tier]}></i>{t('nav.account_line', { elo: r.elo, tier: t('tier.' + r.tier) })}</span>
      </span>
      <img src={avatar(activeAccount().avatar)} alt="" width="32" height="32" />
    </button>
  </div>
</header>

<style>
  .top {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 68px;
    padding: 0 48px;
    display: flex;
    align-items: center;
    gap: 44px;
    z-index: 3;
    color: var(--v-text);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
    font: 700 13px/1 var(--archivo);
    letter-spacing: 0.16em;
    white-space: nowrap;
  }
  .logo { color: var(--v-accent-2); line-height: 0; }
  nav {
    display: flex;
    gap: 28px;
    height: 100%;
  }
  nav button {
    position: relative;
    height: 100%;
    padding: 0;
    border: 0;
    background: none;
    font: 600 12px/1 var(--archivo);
    letter-spacing: 0.16em;
    color: var(--v-muted);
    white-space: nowrap;
    cursor: pointer;
  }
  nav button:hover, nav button.on { color: var(--v-text); }
  nav button.on::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0.16em; /* the tracking after the last letter isn't part of the word */
    bottom: 18px;
    height: 2px;
    background: var(--v-accent-2);
  }
  .me {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 18px;
  }
  .gear {
    padding: 0;
    border: 0;
    background: none;
    color: var(--v-muted);
    line-height: 0;
    cursor: pointer;
  }
  .gear:hover, .gear.on { color: var(--v-text); }
  .acct {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--v-text);
    cursor: pointer;
  }
  .who { display: flex; flex-direction: column; align-items: flex-end; }
  .n { font: 600 13px/1.2 'Inter', sans-serif; }
  .e {
    font: 500 11px/1.2 var(--plex);
    color: var(--v-muted);
    white-space: nowrap;
  }
  .e i {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 6px;
  }
  img {
    width: 32px;
    height: 32px;
    image-rendering: pixelated;
  }
</style>
