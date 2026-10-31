import raw from '../../assets/data.json';

export interface Row {
  rank: number;
  name: string;
  category: string;
  time: string;
  delta: string;
  ahead: boolean;
  elo: number;
  avatar: string;
}
export interface Pace {
  name: string;
  split: string;
  time: string;
  delta: string;
  ahead: boolean;
  avatar: string;
}
export interface Tiles {
  pb: string;
  pb_category: string;
  today_runs: number[];
  elo_history: number[];
  elo: number;
}

export const data = raw as unknown as { rows: Row[]; pace: Pace[]; tiles: Tiles };

// Every avatar becomes a real file in dist/assets (assetsInlineLimit = 0).
const avatarUrls = import.meta.glob<string>('../../assets/avatars/*.png', {
  eager: true,
  query: '?url',
  import: 'default',
});

export function avatar(path: string): string {
  return avatarUrls['../../assets/' + path] ?? '';
}
