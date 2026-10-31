<script lang="ts">
  import { tick } from 'svelte';
  import glowUrl from '../../assets/glow.png';
  import { t, lang } from './copy.svelte';
  import { S } from './screens';
  import { ui, setNavigator, ONB_STEPS, type Page } from './state.svelte';
  import TopNav from './ui/TopNav.svelte';
  import Toast from './ui/Toast.svelte';
  import Home from './Home.svelte';
  import Library from './Library.svelte';
  import Onboarding from './screens/Onboarding.svelte';
  import Accounts from './screens/Accounts.svelte';
  import Modes from './screens/Modes.svelte';
  import CheckModal from './screens/CheckModal.svelte';
  import Practice from './screens/Practice.svelte';
  import Learn from './screens/Learn.svelte';
  import Tools from './screens/Tools.svelte';
  import Stats from './screens/Stats.svelte';
  import Submit from './screens/Submit.svelte';
  import Settings from './screens/Settings.svelte';
  import Game from './screens/Game.svelte';

  // Scene v3 pages that draw their own full-window art and place everything themselves (REDESIGN.md).
  const ART_PAGES: Page[] = ['home', 'modes', 'practice'];

  const isFull = (p: Page) => p === 'onboarding' || p === 'game';

  let shown = $state<Page>('home'); // page currently rendered
  let target: Page = 'home'; // page we are heading to
  let busy = false;
  let glowRunning = $state(false);
  let frameEl: HTMLElement;
  let pageEl: HTMLElement | undefined = $state();
  let libScroller: HTMLElement | undefined = $state();

  const OUT: Keyframe[] = [
    { opacity: 1, transform: 'translateY(0px)' },
    { opacity: 0, transform: 'translateY(8px)' },
  ];
  const IN: Keyframe[] = [
    { opacity: 0, transform: 'translateY(8px)' },
    { opacity: 1, transform: 'translateY(0px)' },
  ];

  /**
   * Page transition: outgoing 150 ms ease-in, then incoming 220 ms ease-out (opacity + translate only).
   * Between shell pages only the page animates (the top nav stays); to/from Onboarding and Game the whole window content does.
   */
  export async function navigate(to: Page): Promise<void> {
    target = to;
    if (busy || to === shown) return;
    busy = true;
    const whole = isFull(to) || isFull(shown);
    const outEl = whole ? frameEl : pageEl!;
    const out = outEl.animate(OUT, { duration: 150, easing: 'ease-in', fill: 'forwards' });
    await out.finished;
    // Take the newest target if it lands in the same kind of layout; otherwise finish this hop first.
    shown = whole === (isFull(target) || isFull(shown)) ? target : to;
    await tick();
    const inEl = whole ? frameEl : pageEl!;
    const inn = inEl.animate(IN, { duration: 220, easing: 'ease-out' });
    out.cancel(); // drop the held fill; IN's first keyframe already covers it
    await inn.finished;
    busy = false;
    if (target !== shown) navigate(target);
  }

  setNavigator((p) => void navigate(p));

  // Window title follows the language (the stack name arrives with the host config).
  $effect(() => {
    if (lang.stack) document.title = t('app.window_title', { stack: lang.stack });
  });

  export function setGlow(on: boolean): void {
    glowRunning = on;
  }

  export function scroller(): HTMLElement | undefined {
    return shown === 'library' ? libScroller : undefined;
  }

  export function current(): Page {
    return target;
  }

  /** Deep link (SCREENS.md "Screen ids"): jump straight to a screen state, no transition. */
  export function openScreen(id: string): boolean {
    let page: Page;
    ui.checkOpen = false;
    if (id.startsWith('onboarding-')) {
      const rest = id.slice('onboarding-'.length);
      const code = rest === 'signin-code';
      const step = ONB_STEPS.indexOf((code ? 'signin' : rest) as (typeof ONB_STEPS)[number]);
      if (step < 0) return false;
      ui.onbStep = step;
      ui.onbCode = code;
      ui.onbDeep = true;
      page = 'onboarding';
    } else if (id === 'check') {
      page = 'home';
      ui.checkMode = '';
      ui.checkOpen = true;
    } else if (/^(home|modes)-/.test(id) && S.modes.some((m) => m.id === id.split('-')[1])) {
      page = id.split('-')[0] as Page; // home-rsg, home-fsg, home-ssg, home-draftout, modes-ssg, ...
      ui.mode = id.split('-')[1];
    } else if (id.startsWith('practice-') && S.practice_categories.some((c) => c.id === id.slice(9))) {
      page = 'practice'; // practice-end, ...
      ui.practiceCat = id.slice(9);
    } else if (
      ['home', 'library', 'accounts', 'modes', 'practice', 'learn', 'tools', 'stats', 'submit', 'settings', 'game'].includes(id)
    ) {
      page = id as Page;
      if (id === 'home') ui.mode = S.v3.default_mode;
      if (id === 'modes') ui.mode = 'ranked';
      if (id === 'practice') ui.practiceCat = S.v3.practice_default;
    } else {
      return false;
    }
    shown = target = page;
    return true;
  }
</script>

<div class="root">
  <div class="frame" bind:this={frameEl}>
    {#if shown === 'onboarding'}
      <Onboarding />
    {:else if shown === 'game'}
      <Game />
    {:else}
      <main class="content">
        <div class="page" class:art={ART_PAGES.includes(shown)} bind:this={pageEl}>
          {#if !ART_PAGES.includes(shown)}
            <img class="glow" class:pulse={glowRunning} src={glowUrl} alt="" draggable="false" />
          {/if}
          {#if shown === 'home'}
            <Home />
          {:else if shown === 'library'}
            <Library bind:scroller={libScroller} />
          {:else if shown === 'accounts'}
            <Accounts />
          {:else if shown === 'modes'}
            <Modes />
          {:else if shown === 'practice'}
            <Practice />
          {:else if shown === 'learn'}
            <Learn />
          {:else if shown === 'tools'}
            <Tools />
          {:else if shown === 'stats'}
            <Stats />
          {:else if shown === 'submit'}
            <Submit />
          {:else if shown === 'settings'}
            <Settings />
          {/if}
        </div>
        <TopNav {shown} onnav={(p) => navigate(p)} />
      </main>
    {/if}
  </div>

  {#if ui.checkOpen}
    <CheckModal />
  {/if}
  <Toast />
</div>

<style>
  .root {
    height: 100%;
    position: relative;
  }
  .frame {
    display: flex;
    height: 100%;
  }

  .content {
    flex: 1 1 auto;
    min-width: 0;
    position: relative;
    overflow: hidden;
    background: var(--bg);
  }

  /* 900x900, centred on the window's top-right corner, under the page content (pages without key art). */
  .glow {
    position: absolute;
    width: 900px;
    height: 900px;
    top: -450px;
    right: -450px;
    pointer-events: none;
    user-select: none;
    opacity: 0.8;
    transform: scale(1);
    z-index: -1;
  }
  .glow.pulse {
    animation: glow-pulse 6s ease-in-out infinite;
  }
  @keyframes glow-pulse {
    0% { opacity: 0.65; transform: scale(1); }
    50% { opacity: 1; transform: scale(1.08); }
    100% { opacity: 0.65; transform: scale(1); }
  }

  /* Shell pages: content 104 px from the top, side padding 64 left and 48 right (REDESIGN.md "Shell"). */
  .page {
    position: relative;
    z-index: 1;
    height: 100%;
    padding: 104px 48px 28px 64px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  /* Key-art pages place everything absolutely in the 1280x800 window. */
  .page.art {
    padding: 0;
    display: block;
  }
</style>
