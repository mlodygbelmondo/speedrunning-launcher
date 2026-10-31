<script lang="ts">
  // Sign-in options (browser / device code), shared by Onboarding and Accounts.
  import { t } from '../copy.svelte';
  import { S } from '../screens';
  import { avatar } from '../data';
  import Icon from './Icon.svelte';
  import Badge from './Badge.svelte';
  import Button from './Button.svelte';
  import Qr from './Qr.svelte';

  let {
    compact = false,
    code = false,
    signed = $bindable(false),
  }: { compact?: boolean; code?: boolean; signed?: boolean } = $props();

  // svelte-ignore state_referenced_locally
  let mode: 'choose' | 'waiting' | 'code' = $state(code ? 'code' : 'choose');
  let timer: ReturnType<typeof setTimeout> | undefined;

  function browser() {
    if (mode !== 'choose') return;
    mode = 'waiting';
    timer = setTimeout(() => {
      mode = 'choose';
      signed = true;
    }, 1500);
  }

  $effect(() => () => clearTimeout(timer));

  const who = $derived(S.accounts.list[0]);
</script>

{#if signed}
  <div class="card success">
    <img class="av" src={avatar(who.avatar)} alt="" width="36" height="36" />
    <div class="grow label">{t('onboarding.signin.done', { name: S.onboarding.signed_in_as })}</div>
    <Icon name="circle-check" size={20} color="var(--ahead)" />
  </div>
{:else if mode === 'code'}
  <div class="card codepanel" class:compact>
    <div class="grow">
      <div class="muted small">{t('onboarding.signin.code_label', { url: S.accounts.device_url })}</div>
      <div class="code mono">{S.accounts.device_code}</div>
      <div class="wait"><Icon name="refresh-cw" size={16} spin />{t('onboarding.signin.waiting')}</div>
    </div>
    <Qr rows={S.accounts.device_qr} module={compact ? 4 : 4.5} />
  </div>
  <div class="back">
    <Button variant="ghost" small icon="chevron-left" onclick={() => (mode = 'choose')}>{t('common.back')}</Button>
  </div>
{:else}
  <div class="opts">
    <button class="card opt" onclick={browser}>
      <span class="tile-icon" style:width="40px" style:height="40px"><Icon name="globe" size={20} /></span>
      <span class="grow">
        <span class="title">{t('onboarding.signin.browser')}<Badge variant="accent">{t('common.recommended')}</Badge></span>
        {#if mode === 'waiting'}
          <span class="hint wait"><Icon name="refresh-cw" size={14} spin />{t('onboarding.signin.waiting')}</span>
        {:else}
          <span class="hint">{t('onboarding.signin.browser_hint')}</span>
        {/if}
      </span>
      <Icon name="chevron-right" size={18} color="var(--muted)" />
    </button>
    <button class="card opt" onclick={() => mode === 'choose' && (mode = 'code')}>
      <span class="tile-icon" style:width="40px" style:height="40px"><Icon name="qr-code" size={20} /></span>
      <span class="grow">
        <span class="title">{t('onboarding.signin.code')}</span>
        <span class="hint">{t('onboarding.signin.code_hint')}</span>
      </span>
      <Icon name="chevron-right" size={18} color="var(--muted)" />
    </button>
  </div>
{/if}

<style>
  .grow { flex: 1 1 auto; min-width: 0; }
  .opts {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .opt {
    width: 100%;
    height: 76px;
    display: flex;
    align-items: center;
    gap: 14px;
    text-align: left;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .opt:hover { border-color: var(--accent); }
  .opt .grow { display: flex; flex-direction: column; gap: 3px; }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    font: 600 15px/20px 'Inter', sans-serif;
  }
  .hint {
    font-size: 13px;
    line-height: 18px;
    color: var(--muted);
  }
  .wait {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    line-height: 18px;
    color: var(--muted);
  }
  .success {
    height: 64px;
    padding-top: 0;
    padding-bottom: 0;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .success .label { font: 600 15px/20px 'Inter', sans-serif; }
  .av {
    width: 36px;
    height: 36px;
    border-radius: 8px;
    image-rendering: pixelated;
  }
  .codepanel {
    display: flex;
    align-items: center;
    gap: 20px;
  }
  .codepanel.compact { gap: 12px; }
  .small { font-size: 13px; line-height: 18px; }
  .code {
    margin: 10px 0 14px;
    font-weight: 700;
    font-size: 32px;
    line-height: 40px;
    letter-spacing: 3px;
    white-space: nowrap;
  }
  .compact .code { font-size: 26px; letter-spacing: 2px; }
  .back { margin-top: 8px; margin-left: -12px; }
</style>
