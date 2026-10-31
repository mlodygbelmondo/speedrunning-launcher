<script lang="ts">
  import { t } from '../copy.svelte';
  import { S, os } from '../screens';
  import { install, installState, toast } from '../state.svelte';
  import Header from '../ui/Header.svelte';
  import Icon from '../ui/Icon.svelte';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import FakeBar from '../ui/FakeBar.svelte';

  let checking = $state(false);
  let checked = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let toggles = $state(S.tools.map((x) => ({ start: x.start_with_game, auto: x.auto_update })));

  function checkNow() {
    if (checking) return;
    checking = true;
    timer = setTimeout(() => {
      checking = false;
      checked = true;
    }, 1000);
  }
  $effect(() => () => clearTimeout(timer));

  const otherOs = (platform: string) => platform !== 'all' && platform !== os;
</script>

<Header title={t('tools.title')} sub={t('tools.subtitle', { ago: checked ? t('tools.checked_now') : S.tools_checked_ago })}>
  {#snippet actions()}
    <Button icon="refresh-cw" spin={checking} onclick={checkNow}>{t('tools.check_now')}</Button>
  {/snippet}
</Header>

<div class="body">
  <div class="card list">
    {#each S.tools as tool, i (tool.id)}
      {@const st = installState('tool:' + tool.id, tool.installed ? 'installed' : 'missing')}
      <div class="row">
        <span class="tile-icon" style:width="44px" style:height="44px"><Icon name={tool.icon} size={20} /></span>
        <div class="text">
          <div class="l1">
            <span class="name" class:muted={otherOs(tool.platform)}>{tool.name}</span>
            {#if tool.version}<span class="mono ver">{tool.version}</span>{/if}
            {#if tool.platform === 'windows'}<Badge>{t('tools.windows_only')}</Badge>{/if}
            {#if tool.platform === 'linux'}<Badge>{t('tools.linux_only')}</Badge>{/if}
          </div>
          <div class="l2">{tool.desc}</div>
          {#if tool.newer}
            <div class="l3 warn"><Icon name="triangle-alert" size={12} />{t('tools.legal_note', { version: tool.newer })}</div>
          {/if}
        </div>
        <div class="right">
          {#if tool.installed}
            <div class="lt">
              <span>{t('tools.start_with_game')}</span>
              <Toggle bind:on={toggles[i].start} label={t('tools.start_with_game')} />
            </div>
            {#if tool.pinned}
              <div class="lt">
                <span>{t('tools.auto_update')}</span>
                <Badge variant="pb" icon="key-round">{t('tools.pinned')}</Badge>
              </div>
            {:else}
              <div class="lt">
                <span>{t('tools.auto_update')}</span>
                <Toggle bind:on={toggles[i].auto} label={t('tools.auto_update')} />
              </div>
            {/if}
          {:else if tool.external}
            <Button small icon="external-link" onclick={() => toast(t('common.opens_in_browser'))}>{t('tools.get', { name: tool.name })}</Button>
          {:else if st === 'installed'}
            <Badge variant="ok" icon="check">{t('common.installed')}</Badge>
          {:else}
            <Badge>{t('tools.not_installed')}</Badge>
            {#if st === 'installing'}
              <Button small disabled>{t('common.installing')}</Button>
            {:else}
              <Button small variant="primary" onclick={() => install('tool:' + tool.id)}>{t('common.install')}</Button>
            {/if}
          {/if}
        </div>
        {#if st === 'installing'}<FakeBar inset={0} />{/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .body {
    flex: 1 1 auto;
    min-height: 0;
  }
  .list { padding: 0 18px; }
  .row {
    position: relative;
    height: 84px;
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .row + .row { border-top: 1px solid var(--border); }
  .text {
    flex: 1 1 auto;
    min-width: 0;
  }
  .l1 {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .name { font: 600 15px/20px 'Inter', sans-serif; }
  .ver { font-size: 12px; color: var(--muted); }
  .l2 {
    margin-top: 2px;
    font-size: 13px;
    line-height: 18px;
    color: var(--muted);
  }
  .l3 {
    margin-top: 2px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    line-height: 16px;
  }
  .right {
    flex: none;
    display: flex;
    align-items: center;
    gap: 24px;
  }
  .right > :global(.badge + .btn) { margin-left: -14px; }
  .lt {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    font-size: 12px;
    line-height: 16px;
    color: var(--muted);
  }
</style>
