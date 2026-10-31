<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { t } from '../copy.svelte';
  import { S } from '../screens';
  import { ui, go, modeById } from '../state.svelte';
  import StatusIcon from '../ui/StatusIcon.svelte';
  import Button from '../ui/Button.svelte';

  const mode = $derived(modeById(ui.checkMode || ui.defaultMode));
  // Only the status lives here; labels and args come from the current language's data.
  let rows = $state(S.checks.map((c) => ({ id: c.id, status: c.status, fixed: false })));
  const args = (i: number) => S.checks[i].args;
  const anyWarn = $derived(rows.some((r) => r.status === 'warn'));
  let launching = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const has = (key: string) => t(key) !== key;
  const close = () => (ui.checkOpen = false);

  function fix(i: number) {
    rows[i].status = 'ok';
    rows[i].fixed = true;
  }

  function launch() {
    close();
    go('game');
  }

  function fixAndLaunch() {
    if (launching) return;
    if (!anyWarn) return launch();
    launching = true;
    rows.forEach((r, i) => r.status === 'warn' && fix(i));
    timer = setTimeout(launch, 400);
  }

  $effect(() => () => clearTimeout(timer));
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={(e) => e.target === e.currentTarget && close()} in:fade={{ duration: 150 }} out:fade={{ duration: 120 }}>
  <div class="card modal" role="dialog" aria-modal="true" in:fly={{ y: 8, duration: 150, easing: cubicOut }} out:fly={{ y: 8, duration: 120 }}>
    <h2>{t('check.title')}</h2>
    <div class="sub">{t('check.subtitle', { mode: mode.name })}</div>
    <div class="rows">
      {#each rows as r, i (r.id)}
        <div class="row">
          <StatusIcon status={r.status === 'warn' ? 'warn' : 'ok'} />
          <div class="txt">
            <span class="label">{t('check.' + r.id)}</span>
            <span class="detail">{t(`check.${r.id}_${r.fixed ? 'fixed' : 'detail'}`, args(i))}</span>
          </div>
          {#if r.status === 'warn' && has(`check.${r.id}_action`)}
            <Button small onclick={() => fix(i)}>{t(`check.${r.id}_action`, args(i))}</Button>
          {/if}
        </div>
      {/each}
    </div>
    <div class="foot">
      <div class="note">{t('check.footer')}</div>
      <Button variant="ghost" onclick={close}>{t('common.cancel')}</Button>
      {#if anyWarn}
        <Button variant="primary" onclick={fixAndLaunch}>{t('check.fix_and_launch')}</Button>
      {:else}
        <Button variant="primary" onclick={fixAndLaunch}>{t('check.launch')}</Button>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(5 4 8 / 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .modal {
    width: 600px;
    padding: 24px;
    border-radius: 16px;
    box-shadow: 0 24px 64px rgb(0 0 0 / 0.5);
  }
  h2 {
    margin: 0;
    font: 700 20px/28px 'Inter', sans-serif;
  }
  .sub {
    margin-top: 2px;
    font-size: 13px;
    line-height: 18px;
    color: var(--muted);
  }
  .rows { margin-top: 12px; }
  .row {
    min-height: 52px;
    padding: 8px 0;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .row + .row { border-top: 1px solid var(--border); }
  .txt {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: 12px;
  }
  .label { font: 600 14px/20px 'Inter', sans-serif; white-space: nowrap; }
  .detail { font-size: 13px; line-height: 18px; color: var(--muted); }
  .foot {
    margin-top: 16px;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .note {
    flex: 1 1 auto;
    min-width: 0;
    font-size: 12px;
    line-height: 16px;
    color: var(--muted);
  }
</style>
