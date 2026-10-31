<script lang="ts">
  import type { Snippet } from 'svelte';
  import { t, lang, LANGS } from '../copy.svelte';
  import { S, os } from '../screens';
  import { ui, go, toast } from '../state.svelte';
  import Header from '../ui/Header.svelte';
  import Icon from '../ui/Icon.svelte';
  import Button from '../ui/Button.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';

  const st = ui.settings;

  function runSetup() {
    ui.onbStep = 0;
    ui.onbCode = false;
    ui.onbDeep = false;
    go('onboarding');
  }
</script>

{#snippet row(label: string, control: Snippet, hint?: string)}
  <div class="srow">
    <div class="grow">
      <div class="slabel">{label}</div>
      {#if hint}<div class="shint">{hint}</div>{/if}
    </div>
    {@render control()}
  </div>
{/snippet}

<Header title={t('settings.title')} sub={t('settings.subtitle')} />

<div class="body">
  <div class="col">
    <section class="card">
      <h2 class="card-title bullet">{t('settings.section_general')}</h2>
      {#snippet langSeg()}
        <Segmented items={S.settings.languages} value={LANGS.indexOf(lang.cur)} onchange={(i) => (lang.cur = LANGS[i])} />
      {/snippet}
      {#snippet close()}<Toggle bind:on={st.close_on_launch} label={t('settings.close_on_launch')} />{/snippet}
      {#snippet boot()}<Toggle bind:on={st.start_with_system} label={t('settings.start_with_system')} />{/snippet}
      {@render row(t('settings.language'), langSeg)}
      {@render row(t('settings.close_on_launch'), close, t('settings.close_on_launch_hint'))}
      {@render row(t('settings.start_with_system'), boot)}
    </section>
    <section class="card">
      <h2 class="card-title bullet">{t('settings.section_game')}</h2>
      {#snippet java()}<span class="value">{t('settings.java_value')}</span><Icon name="circle-check" size={16} color="var(--ahead)" />{/snippet}
      {#snippet mem()}<Segmented items={S.settings.memory} bind:value={st.memory} />{/snippet}
      {@render row(t('settings.java'), java)}
      {@render row(t('settings.memory'), mem, t('settings.memory_hint'))}
    </section>
    <section class="card about">
      <h2 class="card-title bullet">{t('settings.section_about')}</h2>
      <p>{t('settings.disclaimer')}</p>
      <p>{t('settings.credits')}</p>
    </section>
  </div>

  <div class="col">
    <section class="card">
      <h2 class="card-title bullet">{t('settings.section_controls')}</h2>
      {#snippet keys()}<Segmented items={S.settings.keybinds} bind:value={st.keybinds} />{/snippet}
      {#snippet sens()}<span class="box mono">{S.settings.sensitivity}</span>{/snippet}
      {#snippet stdset()}<Toggle bind:on={st.standard_settings} label={t('settings.standard_settings')} />{/snippet}
      {@render row(t('settings.keybinds'), keys)}
      {@render row(t('settings.sensitivity'), sens)}
      {@render row(t('settings.standard_settings'), stdset)}
    </section>
    <section class="card">
      <h2 class="card-title bullet">{t('settings.section_updates')}</h2>
      {#snippet auto()}<Toggle bind:on={st.auto_update} label={t('settings.auto_update')} />{/snippet}
      {#snippet chan()}<Segmented items={S.settings.channels} bind:value={st.channel} />{/snippet}
      {@render row(t('settings.auto_update'), auto)}
      {@render row(t('settings.channel'), chan)}
    </section>
    <section class="card">
      <h2 class="card-title bullet">{t('settings.section_advanced')}</h2>
      {#snippet folder()}
        <span class="mono path">{S.settings.data_folder[os]}</span>
        <Button variant="ghost" small icon="folder-open">{t('common.open_folder')}</Button>
      {/snippet}
      {#snippet setup()}<Button small onclick={runSetup}>{t('settings.run_setup')}</Button>{/snippet}
      {#snippet reset()}<Button small variant="danger" onclick={() => toast(t('settings.reset_all'))}>{t('settings.reset_all')}</Button>{/snippet}
      {@render row(t('settings.data_folder'), folder)}
      {@render row(t('settings.run_setup'), setup)}
      {@render row(t('settings.reset_all'), reset)}
    </section>
  </div>
</div>

<style>
  .body {
    flex: 1 1 auto;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 20px;
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  /* Row lists bring their own vertical space, so the card's bottom padding is trimmed. */
  .card { padding-bottom: 4px; }
  .card-title { margin-bottom: 4px; }
  /* 50 px rows and 12 px card gaps: the v3 top nav starts the page at 104 px and About was added. */
  .srow {
    min-height: 50px;
    padding: 6px 0;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .srow + .srow { border-top: 1px solid var(--border); }
  .grow { flex: 1 1 auto; min-width: 0; }
  .slabel { font: 500 14px/20px 'Inter', sans-serif; }
  .shint { margin-top: 2px; font-size: 12px; line-height: 16px; color: var(--muted); }
  .value { font-size: 13px; color: var(--muted); }
  .box {
    width: 64px;
    height: 32px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg);
    font-size: 13px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .about { padding-bottom: 16px; }
  .about p { margin: 8px 0 0; font-size: 12px; line-height: 1.45; color: var(--v-faint); }
  .path { font-size: 12px; color: var(--muted); white-space: nowrap; }
  .srow :global(.btn.ghost:last-child) { margin-right: -12px; }
</style>
