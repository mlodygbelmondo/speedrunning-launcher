// Scene v3 key art (assets/art, REDESIGN.md "Art"). Every file is a real asset in dist (one decode, cached by the
// browser). Ids: modes ranked/rsg/fsg/ssg/draftout, categories overworld/nether-portal/bastion/fortress/stronghold/the-end.
import shadeUrl from '../../assets/art/shade-hero.png';

const glob = (g: Record<string, string>) =>
  Object.fromEntries(Object.entries(g).map(([p, url]) => [p.slice(p.lastIndexOf('/') + 1, -4), url]));

const heroes = glob(import.meta.glob<string>('../../assets/art/hero/*.jpg', { eager: true, query: '?url', import: 'default' }));
const cards = glob(import.meta.glob<string>('../../assets/art/card/*.jpg', { eager: true, query: '?url', import: 'default' }));

export const hero = (id: string): string => heroes[id] ?? '';
export const card = (id: string): string => cards[id] ?? '';
export const shade = shadeUrl;

/** Caps labels: copy is normal case, code uppercases (Unicode-aware). */
export const up = (s: string): string => s.toUpperCase();

/** MCSR Ranked tier colours (REDESIGN.md "Tokens"). */
export const TIER_COLOR: Record<string, string> = {
  coal: '#AAAAAA', iron: '#FFFFFF', gold: '#EAB308', emerald: '#21A83B', diamond: '#2CE0D8', netherite: '#7C5CBF',
};
