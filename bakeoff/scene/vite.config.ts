import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// scene/dist must be self-contained: assets from ../assets are imported (fonts, hero, glow,
// data.json, avatars via import.meta.glob) so Vite copies them into dist/assets.
export default defineConfig({
  base: './', // relative URLs: works from tauri://, file:// (Electron) and http://
  plugins: [svelte()],
  server: { fs: { allow: ['..'] } },
  build: {
    target: 'es2022',
    assetsInlineLimit: 0, // keep every image/font as a real file (no data: URIs)
    chunkSizeWarningLimit: 2000,
  },
});
