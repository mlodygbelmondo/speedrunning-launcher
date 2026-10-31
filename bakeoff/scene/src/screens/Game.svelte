<script lang="ts">
  import glowUrl from '../../../assets/glow.png';
  import { t } from '../copy.svelte';
  import { S } from '../screens';
  import { go, toast } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
  import Button from '../ui/Button.svelte';

  const g = $derived(S.game);
  const minis = $derived([
    { label: 'game.resets', value: g.resets, pb: false },
    { label: 'game.best', value: g.best, pb: true },
    { label: 'game.nether_entries', value: g.nether_entries, pb: false },
  ]);
</script>

<div class="game">
  <img class="glow" src={glowUrl} alt="" draggable="false" />

  <div class="card centre">
    <div class="trow">
      <span class="sq"></span>
      <h1>{t('game.title')}</h1>
    </div>
    <div class="sub">{t('game.subtitle', { mode: g.mode, ago: g.started })}</div>
    <h2 class="card-title bullet session">{t('game.session')}</h2>
    <div class="minis">
      {#each minis as m (m.label)}
        <div class="mini">
          <div class="mlabel">{t(m.label)}</div>
          <div class="mono mvalue" class:pbc={m.pb}>{m.value}</div>
        </div>
      {/each}
    </div>
    <div class="actions">
      <Button variant="danger" icon="circle-x" onclick={() => go('home')}>{t('game.stop')}</Button>
      <div class="wnote">
        <Icon name="info" size={16} color="var(--muted)" />
        <span>{t('game.window_note')}</span>
      </div>
    </div>
  </div>

  <div class="traywrap">
    <div class="caption">{t('game.tray_title')}</div>
    <div class="menu">
      <div class="mhead">
        <span class="accent"><Icon name="logo" size={16} /></span>
        <span>{t('tray.status', { mode: g.mode })}</span>
      </div>
      <div class="line"></div>
      <button class="item" onclick={() => go('home')}><Icon name="house" size={16} />{t('tray.show')}</button>
      <button class="item" onclick={() => toast(t('tray.ninjabrain'))}><Icon name="eye" size={16} />{t('tray.ninjabrain')}</button>
      <div class="line"></div>
      <button class="item stop" onclick={() => go('home')}><Icon name="circle-x" size={16} />{t('tray.stop')}</button>
      <button class="item" onclick={() => toast(t('tray.quit'))}><Icon name="power" size={16} />{t('tray.quit')}</button>
    </div>
  </div>

  <div class="taskbar">
    <Icon name="globe" size={16} />
    <Icon name="video" size={16} />
    <Icon name="monitor" size={16} />
    <span class="ours"><Icon name="logo" size={16} /></span>
    <span class="mono clock">21:37</span>
  </div>
</div>

<style>
  .game {
    position: relative;
    flex: 1 1 auto;
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
  .centre {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 620px;
    padding: 28px;
    transform: translate(-50%, calc(-50% - 40px));
  }
  .trow {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .sq {
    flex: none;
    width: 10px;
    height: 10px;
    background: var(--ahead);
  }
  h1 {
    margin: 0;
    font: 700 28px/36px 'Inter', sans-serif;
  }
  .sub {
    margin-top: 4px;
    font-size: 13px;
    line-height: 18px;
    color: var(--muted);
  }
  .session { margin-top: 24px; }
  .minis {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }
  .mini {
    height: 84px;
    padding: 14px;
    border-radius: 12px;
    background: var(--raised);
  }
  .mlabel { font-size: 12px; line-height: 16px; color: var(--muted); }
  .mvalue {
    margin-top: 8px;
    font-weight: 700;
    font-size: 22px;
    line-height: 28px;
  }
  .actions {
    margin-top: 24px;
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .wnote {
    flex: 1 1 auto;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 12px;
    line-height: 16px;
    color: var(--muted);
  }

  .taskbar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 36px;
    padding-right: 16px;
    background: #050408;
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 14px;
    color: var(--muted);
  }
  .ours {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    background: var(--raised);
    color: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .clock { font-size: 12px; }

  /* Right edge of the menu = right edge of our tray icon: 16 px padding + clock (5 ch of 12 px mono) + 14 px gap. */
  .traywrap {
    position: absolute;
    bottom: 44px;
    right: calc(16px + 5ch + 14px);
    width: 264px;
    font-size: 12px; /* makes 5ch the clock width */
    font-family: var(--mono);
  }
  .caption {
    margin-bottom: 6px;
    text-align: right;
    font: 400 11px/14px 'Inter', sans-serif;
    color: var(--muted);
  }
  .menu {
    padding: 6px;
    border-radius: 12px;
    border: 1px solid var(--border);
    background: var(--raised);
    box-shadow: 0 16px 40px rgb(0 0 0 / 0.5);
    font-family: 'Inter', sans-serif;
  }
  .mhead {
    height: 40px;
    padding: 0 10px;
    display: flex;
    align-items: center;
    gap: 10px;
    font: 600 13px/18px 'Inter', sans-serif;
  }
  .accent { color: var(--accent); line-height: 0; }
  .line {
    height: 1px;
    margin: 4px 0;
    background: var(--border);
  }
  .item {
    width: 100%;
    height: 36px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text);
    display: flex;
    align-items: center;
    gap: 10px;
    font: 400 13px/18px 'Inter', sans-serif;
    text-align: left;
    cursor: pointer;
  }
  .item :global(.icon) { color: var(--muted); }
  .item:hover { background: var(--surface); }
  .item.stop { color: var(--behind); }
</style>
