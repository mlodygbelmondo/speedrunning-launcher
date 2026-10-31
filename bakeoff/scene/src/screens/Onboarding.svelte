<script lang="ts">
  import glowUrl from '../../../assets/glow.png';
  import heroUrl from '../../../assets/hero.jpg';
  import { t } from '../copy.svelte';
  import { S, os } from '../screens';
  import { ui, go, ONB_STEPS } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import SignIn from '../ui/SignIn.svelte';

  const last = ONB_STEPS.length - 1;
  const step = $derived(ui.onbStep);
  const id = $derived(ONB_STEPS[step]);

  // Every step's state lives here, so it survives Back/Next.
  let signed = $state(false);
  let mode = $state((S.modes.find((m) => m.recommended) ?? S.modes[0]).id);
  let memory = $state(S.settings.memory_default);
  let eyeWin = $state(os === 'windows');
  let eyeLinux = $state(os === 'linux');
  let eyeNinja = $state(true);
  let obsAdd = $state(true);

  const modeName = $derived(S.modes.find((m) => m.id === mode)?.name ?? '');
  const toolName = (tid: string) => S.tools.find((x) => x.id === tid)?.name ?? tid;
  const toolsSummary = $derived(
    [os === 'linux' ? eyeLinux && toolName('waywall') : eyeWin && toolName('toolscreen'), eyeNinja && toolName('ninjabrain')]
      .filter(Boolean)
      .join(', '),
  );
  const nextBlocked = $derived(id === 'signin' && !signed && !ui.onbDeep);

  const next = () => (ui.onbStep = Math.min(last, step + 1));
  const back = () => (ui.onbStep = Math.max(0, step - 1));
</script>

<div class="onb">
  <img class="glow" src={glowUrl} alt="" draggable="false" />

  <aside class="left">
    <div class="brand">
      <span class="logo"><Icon name="logo" size={28} /></span>
      <span>Speedrunluncher</span>
    </div>
    <ol class="steps">
      {#each ONB_STEPS as s, i (s)}
        <li class:done={i < step} class:cur={i === step}>
          <span class="marker"></span>
          <span class="label">{t('onboarding.steps.' + s)}</span>
          {#if i < step}<Icon name="check" size={14} color="var(--ahead)" />{/if}
        </li>
      {/each}
    </ol>
    <div class="stepof">{t('onboarding.step_of', { n: step + 1, total: ONB_STEPS.length })}</div>
  </aside>

  <section class="right">
    <div class="col">
      {#if id === 'welcome'}
        <div class="banner" style:background-image="url({heroUrl})"></div>
      {:else if id === 'done'}
        <div class="done-tile"><Icon name="check" size={32} /></div>
      {/if}
      <h1>{t(`onboarding.${id}.title`)}</h1>
      <p class="body">{t(`onboarding.${id}.body`, { mode: modeName })}</p>

      <div class="content">
        {#if id === 'signin'}
          {#key ui.onbCode}
            <SignIn code={ui.onbCode} bind:signed />
          {/key}
        {:else if id === 'mode'}
          <div class="card list">
            {#each S.modes as m (m.id)}
              <button class="mode-row" class:sel={mode === m.id} onclick={() => (mode = m.id)}>
                <span class="tile-icon" style:width="36px" style:height="36px"><Icon name={m.icon} size={18} /></span>
                <span class="grow">
                  <span class="line1">
                    <span class="name">{m.name}</span>
                    {#if m.recommended}<Badge variant="accent">{t('common.recommended')}</Badge>{/if}
                    <span class="full">{m.full}</span>
                  </span>
                  <span class="desc">{m.desc}</span>
                </span>
                <Badge mono>{m.mc}</Badge>
                <span class="radio" class:on={mode === m.id}></span>
              </button>
            {/each}
          </div>
        {:else if id === 'java'}
          <div class="card rows">
            <div class="srow">
              <div class="grow">
                <div class="slabel">{t('onboarding.java.java_label')}</div>
              </div>
              <span class="value">{t('onboarding.java.java_value')}</span>
              <Icon name="circle-check" size={16} color="var(--ahead)" />
            </div>
            <div class="srow">
              <div class="grow">
                <div class="slabel">{t('onboarding.java.memory_label')}</div>
                <div class="shint">{t('onboarding.java.memory_hint')}</div>
              </div>
              <Segmented items={S.settings.memory} bind:value={memory} />
            </div>
          </div>
        {:else if id === 'eye'}
          <div class="card rows">
            <div class="crow" class:off={os !== 'windows'}>
              <Checkbox bind:checked={eyeWin} />
              <span class="grow">{t('onboarding.eye.windows')}</span>
              <Badge>{t('tools.windows_only')}</Badge>
            </div>
            <div class="crow" class:off={os !== 'linux'}>
              <Checkbox bind:checked={eyeLinux} />
              <span class="grow">{t('onboarding.eye.linux')}</span>
              <Badge>{t('tools.linux_only')}</Badge>
            </div>
            <div class="crow">
              <Checkbox bind:checked={eyeNinja} />
              <span class="grow">{t('onboarding.eye.ninjabrain')}</span>
            </div>
          </div>
        {:else if id === 'obs'}
          <div class="card rows">
            <div class="srow">
              <Icon name="circle-check" size={20} color="var(--ahead)" />
              <div class="grow slabel">{t('onboarding.obs.found', { version: S.onboarding.obs_version })}</div>
            </div>
            <div class="srow">
              <div class="grow slabel">{t('onboarding.obs.add')}</div>
              <Toggle bind:on={obsAdd} />
            </div>
          </div>
          <div class="under">
            <Button variant="ghost" onclick={() => (obsAdd = false)}>{t('onboarding.obs.no_obs')}</Button>
          </div>
        {:else if id === 'done'}
          <div class="card rows summary">
            <div class="sumrow"><span class="muted">{t('rail.modes')}</span><span>{modeName}</span></div>
            <div class="sumrow">
              <span class="muted">{t('onboarding.java.java_label')}</span>
              <span>{S.layers.java.detail} · {S.settings.memory[memory]}</span>
            </div>
            <div class="sumrow"><span class="muted">{t('rail.tools')}</span><span>{toolsSummary}</span></div>
          </div>
        {/if}
      </div>
    </div>

    <footer class="foot">
      <div class="footrow">
        {#if id !== 'welcome' && id !== 'done'}
          <Button variant="ghost" icon="chevron-left" onclick={back}>{t('common.back')}</Button>
        {/if}
        <div class="grow"></div>
        {#if id === 'welcome'}
          <Button variant="primary" onclick={next}>{t('onboarding.welcome.cta')}</Button>
        {:else if id === 'done'}
          <Button onclick={() => go('learn')}>{t('onboarding.done.learn')}</Button>
          <Button variant="primary" onclick={() => go('home')}>{t('onboarding.done.launch', { mode: modeName })}</Button>
        {:else}
          {#if id === 'eye'}
            <Button variant="ghost" onclick={next}>{t('onboarding.eye.later')}</Button>
          {/if}
          <Button variant="primary" iconAfter="chevron-right" disabled={nextBlocked} onclick={next}
            >{t('common.next')}</Button
          >
        {/if}
      </div>
      {#if id === 'welcome'}
        <div class="have">
          <Button variant="ghost">{t('onboarding.welcome.have_setup')}</Button>
        </div>
      {/if}
    </footer>
  </section>
  {#if id === 'welcome'}<p class="disc">{t('settings.disclaimer')}</p>{/if}
</div>

<style>
  .onb {
    position: relative;
    flex: 1 1 auto;
    display: flex;
    height: 100%;
    overflow: hidden;
    background: var(--bg);
  }
  .glow {
    position: absolute;
    width: 900px;
    height: 900px;
    top: -450px;
    right: -450px;
    opacity: 0.8;
    pointer-events: none;
  }
  .grow { flex: 1 1 auto; min-width: 0; }
  /* Mojang usage guidelines (REDESIGN.md "Disclaimer and credits"): bottom of the window, centred under the
     content column (right of the 340 px step panel, so it never runs into the panel's edge). */
  .disc {
    position: absolute;
    z-index: 2;
    left: 340px;
    right: 0;
    bottom: 12px;
    margin: 0;
    font-size: 11px;
    line-height: 16px;
    text-align: center;
    color: var(--v-faint);
    pointer-events: none;
  }

  .left {
    position: relative;
    z-index: 1;
    flex: 0 0 340px;
    padding: 32px;
    background: var(--surface);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font: 700 18px/28px 'Inter', sans-serif;
  }
  .logo { color: var(--accent); line-height: 0; }
  .steps {
    list-style: none;
    margin: 40px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .steps li {
    height: 40px;
    padding: 0 12px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--muted);
  }
  .marker {
    flex: none;
    width: 10px;
    height: 10px;
    background: var(--border);
  }
  .label {
    flex: 1 1 auto;
    font: 500 14px/20px 'Inter', sans-serif;
  }
  .done .marker { background: var(--accent); }
  .cur {
    background: var(--raised);
    color: var(--text);
  }
  .cur .marker {
    background: var(--accent);
    box-shadow: 0 0 0 2px rgb(139 92 246 / 0.3);
  }
  .stepof {
    margin-top: auto;
    font-size: 12px;
    line-height: 16px;
    color: var(--muted);
  }

  .right {
    position: relative;
    z-index: 1;
    flex: 1 1 auto;
    min-width: 0;
  }
  .col {
    width: 600px;
    margin: 0 auto;
    padding-top: 72px;
  }
  .banner {
    height: 180px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background-size: cover;
    background-position: center;
    margin-bottom: 24px;
  }
  .done-tile {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: rgb(139 92 246 / 0.18);
    color: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 24px;
  }
  h1 {
    margin: 0;
    font: 700 28px/36px 'Inter', sans-serif;
  }
  .body {
    margin: 10px 0 0;
    max-width: 560px;
    font-size: 15px;
    line-height: 22px;
    color: var(--muted);
  }
  .content { margin-top: 28px; }

  .list { padding: 0; overflow: hidden; }
  .mode-row {
    width: 100%;
    min-height: 64px; /* grows when the description needs a second line */
    padding: 10px 18px;
    border: 0;
    background: transparent;
    color: var(--text);
    display: flex;
    align-items: center;
    gap: 12px;
    text-align: left;
    cursor: pointer;
  }
  .mode-row + .mode-row { border-top: 1px solid var(--border); }
  .mode-row:hover { background: rgb(27 19 39 / 0.6); }
  .mode-row.sel { background: rgb(139 92 246 / 0.1); }
  .mode-row .grow { display: flex; flex-direction: column; gap: 2px; }
  .line1 { display: flex; align-items: center; gap: 8px; white-space: nowrap; }
  .name { font: 600 15px/20px 'Inter', sans-serif; }
  .full { font-size: 12px; color: var(--muted); }
  .desc {
    font-size: 13px;
    line-height: 18px;
    color: var(--muted);
  }
  .radio {
    flex: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1.5px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .radio.on {
    border: 0;
    background: var(--accent);
  }
  .radio.on::after {
    content: '';
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text);
  }

  .rows { padding-top: 0; padding-bottom: 0; }
  .srow {
    min-height: 56px;
    padding: 8px 0;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .srow + .srow, .crow + .crow, .sumrow + .sumrow { border-top: 1px solid var(--border); }
  .slabel { font: 500 14px/20px 'Inter', sans-serif; }
  .shint { margin-top: 2px; font-size: 12px; line-height: 16px; color: var(--muted); }
  .value { font-size: 13px; color: var(--muted); }
  .crow {
    height: 56px;
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 14px;
  }
  .crow.off .grow { color: var(--muted); }
  .under { margin-top: 8px; margin-left: -18px; }
  .sumrow {
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 14px;
  }

  .foot {
    position: absolute;
    bottom: 40px;
    left: 50%;
    width: 600px;
    margin-left: -300px;
  }
  .footrow {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .footrow > :global(.btn.ghost:first-child) { margin-left: -18px; }
  .have { margin-top: 8px; margin-left: -18px; }
</style>
