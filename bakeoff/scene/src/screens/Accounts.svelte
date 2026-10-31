<script lang="ts">
  import { t } from '../copy.svelte';
  import { S, os } from '../screens';
  import { avatar } from '../data';
  import { ui } from '../state.svelte';
  import Header from '../ui/Header.svelte';
  import Icon from '../ui/Icon.svelte';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import SignIn from '../ui/SignIn.svelte';

  let highlight = $state(false);
  let addCard: HTMLElement;
  let hlTimer: ReturnType<typeof setTimeout> | undefined;

  function use(i: number) {
    ui.accounts.forEach((a, j) => (a.active = j === i));
    const a = ui.accounts[i];
    if (a.expired) {
      a.expired = false;
      a.fresh = true;
    }
  }

  function signOut(i: number) {
    if (ui.accounts.length < 2) return; // keep one account so the rail always has an avatar
    const wasActive = ui.accounts[i].active;
    ui.accounts.splice(i, 1);
    if (wasActive) ui.accounts[0].active = true;
  }

  function add() {
    addCard.scrollIntoView({ block: 'nearest' });
    highlight = true;
    clearTimeout(hlTimer);
    hlTimer = setTimeout(() => (highlight = false), 600);
  }

  $effect(() => () => clearTimeout(hlTimer));
</script>

<Header title={t('accounts.title')} sub={t('accounts.subtitle')} />

<div class="body">
  <div class="left">
    <div class="card list">
      {#each ui.accounts as a, i (a.name)}
        <div class="acc">
          <img class="av" src={avatar(a.avatar)} alt="" width="44" height="44" />
          <div class="who">
            <div class="name">{a.name}</div>
            {#if a.expired}
              <div class="st warn">{t('accounts.status_expired')}</div>
            {:else}
              <div class="st muted">{t('accounts.status_ok', { ago: a.fresh ? t('tools.checked_now') : (S.accounts.list.find((x) => x.name === a.name)?.ago ?? '') })}</div>
            {/if}
          </div>
          {#if a.active}
            <Badge variant="ok">{t('accounts.active')}</Badge>
          {:else}
            <Button small onclick={() => use(i)}>{a.expired ? t('accounts.sign_in_again') : t('accounts.set_active')}</Button>
          {/if}
          <Button small variant="ghost" onclick={() => signOut(i)}>{t('accounts.sign_out')}</Button>
        </div>
      {/each}
    </div>
    <div class="addrow">
      <Button icon="plus" onclick={add}>{t('accounts.add')}</Button>
    </div>
    <div class="note">
      <Icon name="key-round" size={16} color="var(--muted)" />
      <span>{t('accounts.storage_note', { store: S.accounts.store[os] })}</span>
    </div>
  </div>

  <div class="right">
    <div class="card" class:hl={highlight} bind:this={addCard}>
      <h2 class="card-title">{t('accounts.add_title')}</h2>
      <SignIn compact />
    </div>
  </div>
</div>

<style>
  .body {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    gap: 20px;
  }
  .left {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .right { flex: 0 0 440px; width: 440px; min-width: 0; }
  .list { padding-top: 0; padding-bottom: 0; }
  .acc {
    height: 72px;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .acc + .acc { border-top: 1px solid var(--border); }
  .av {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    image-rendering: pixelated;
    margin-right: 4px;
  }
  .who { flex: 1 1 auto; min-width: 0; }
  .name { font: 600 15px/20px 'Inter', sans-serif; }
  .st { margin-top: 2px; font-size: 12px; line-height: 16px; }
  .addrow { margin-top: 16px; }
  .note {
    margin-top: auto;
    max-width: 520px;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 12px;
    line-height: 16px;
    color: var(--muted);
  }
  .card.hl { border-color: var(--accent); }
</style>
