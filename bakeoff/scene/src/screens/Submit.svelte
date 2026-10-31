<script lang="ts">
  import { t } from '../copy.svelte';
  import { S } from '../screens';
  import { go, toast } from '../state.svelte';
  import Header from '../ui/Header.svelte';
  import Icon from '../ui/Icon.svelte';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';

  const sb = $derived(S.submit);
  let link = $state('');
  let ruleVideo = $state(false);
  let ruleFiles = $state(false);
  const ready = $derived(link.trim() !== '' && ruleVideo && ruleFiles);
</script>

<Header title={t('submit.title')} sub={t('submit.subtitle', { time: sb.time, category: sb.category, date: sb.date })}>
  {#snippet above()}
    <Button variant="ghost" small icon="chevron-left" onclick={() => go('stats')}>{t('rail.stats')}</Button>
  {/snippet}
</Header>

<div class="body">
  <div class="left">
    <div class="card">
      <h2 class="card-title bullet">{t('submit.files')}</h2>
      {#each sb.files as [label, file] (label)}
        <div class="frow">
          <Icon name="file-check" size={16} color="var(--ahead)" />
          <span class="grow">{label}</span>
          <span class="mono fname">{file}</span>
        </div>
      {/each}
      <div class="zip">
        <Icon name="folder-open" size={16} color="var(--muted)" />
        <span class="mono zname">{sb.zip}</span>
        <span class="zsize">{sb.zip_size}</span>
        <Badge variant="ok">{t('submit.files_ready')}</Badge>
        <span class="grow"></span>
        <Button variant="ghost" small>{t('submit.show_folder')}</Button>
      </div>
    </div>

    <div class="card">
      <h2 class="card-title bullet">{t('submit.timing')}</h2>
      <div class="timing">
        <div>
          <div class="tlabel">{t('submit.igt')}</div>
          <div class="mono igt">{sb.igt}</div>
        </div>
        <div>
          <div class="tlabel">{t('submit.rta')}</div>
          <div class="mono rta">{sb.rta}</div>
        </div>
      </div>
    </div>
  </div>

  <div class="right">
    <div class="card">
      <h2 class="card-title bullet">{t('submit.video')}</h2>
      <div class="vrow">
        <input class="field" class:filled={link !== ''} bind:value={link} placeholder={t('submit.video_placeholder')} spellcheck="false" />
        <Button onclick={() => (link = sb.sample_link)}>{t('submit.paste')}</Button>
      </div>
    </div>

    <div class="card">
      <h2 class="card-title bullet">{t('submit.checklist')}</h2>
      <div class="crow">
        <Checkbox bind:checked={ruleVideo} label={t('submit.rule_video')} />
        <span>{t('submit.rule_video')}</span>
      </div>
      <div class="crow">
        <Checkbox bind:checked={ruleFiles} label={t('submit.rule_files')} />
        <span>{t('submit.rule_files')}</span>
      </div>
      <div class="warnnote">
        <Icon name="triangle-alert" size={16} />
        <span>{t('submit.rule_launcher')}</span>
      </div>
    </div>

    <div class="btns">
      <Button icon="copy" onclick={() => toast(t('submit.copied'))}>{t('submit.copy')}</Button>
      <Button variant="primary" icon="external-link" disabled={!ready} onclick={() => toast(t('common.opens_in_browser'))}
        >{t('submit.open_src')}</Button
      >
    </div>
  </div>
</div>

<style>
  .body {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    gap: 20px;
    align-items: flex-start;
  }
  .left, .right {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .left { flex: 1 1 auto; min-width: 0; }
  .right { flex: 0 0 480px; }
  .grow { flex: 1 1 auto; min-width: 0; }
  .frow {
    height: 40px;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 14px;
  }
  .fname { font-size: 12px; color: var(--muted); }
  .zip {
    margin-top: 6px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .zname { font-size: 13px; }
  .zsize { font-size: 12px; color: var(--muted); }
  .zip :global(.btn.ghost) { margin-right: -12px; }
  .timing {
    display: flex;
    align-items: flex-end;
    gap: 48px;
  }
  .tlabel { font-size: 12px; line-height: 16px; color: var(--muted); }
  .igt {
    margin-top: 4px;
    font-weight: 700;
    font-size: 30px;
    line-height: 38px;
    color: var(--pb);
  }
  .rta {
    margin-top: 4px;
    font-weight: 700;
    font-size: 22px;
    line-height: 34px;
  }
  .vrow {
    display: flex;
    gap: 10px;
  }
  .field {
    flex: 1 1 auto;
    min-width: 0;
    height: 40px;
    padding: 0 12px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    font: 400 14px 'Inter', sans-serif;
    outline: none;
  }
  .field::placeholder { color: var(--muted); }
  .field:focus { border-color: var(--muted); }
  .field.filled {
    border-color: var(--accent);
    font: 400 13px var(--mono);
  }
  .crow {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 6px 0;
    font-size: 13px;
    line-height: 18px;
  }
  .warnnote {
    margin-top: 10px;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgb(251 191 36 / 0.08);
    color: var(--warn);
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: 13px;
    line-height: 18px;
  }
  .warnnote :global(.icon) { margin-top: 1px; }
  .btns {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
