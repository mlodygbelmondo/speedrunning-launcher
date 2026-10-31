// Cross-platform packaging (Linux, Windows, macOS): stages main.js + preload.js + ../scene/dist
// (as scene/) and runs @electron/packager -> out/bakeoff-electron-<platform>-<arch>/.
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { packager } from '@electron/packager';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const sceneDist = join(root, '..', 'scene', 'dist');
const outDir = join(root, 'out');
const stage = join(outDir, '_stage');

if (!existsSync(join(sceneDist, 'index.html'))) {
  console.error(`scene not built: ${sceneDist}/index.html missing. Run "pnpm build" in bakeoff/scene first.`);
  process.exit(1);
}

const require = createRequire(import.meta.url);
const electronVersion = JSON.parse(readFileSync(require.resolve('electron/package.json'), 'utf8')).version;

rmSync(stage, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });
for (const f of ['main.js', 'preload.js']) cpSync(join(root, f), join(stage, f));
cpSync(sceneDist, join(stage, 'scene'), { recursive: true });
writeFileSync(
  join(stage, 'package.json'),
  JSON.stringify({ name: 'bakeoff-electron', productName: 'bakeoff-electron', version: '0.1.0', main: 'main.js' }, null, 2),
);

const paths = await packager({
  dir: stage,
  out: outDir,
  name: 'bakeoff-electron',
  executableName: 'bakeoff-electron',
  platform: process.platform,
  arch: process.arch,
  electronVersion,
  asar: false, // scene must be at resources/app/scene
  prune: false,
  overwrite: true,
  quiet: true,
});
rmSync(stage, { recursive: true, force: true });
console.log(`packaged electron ${electronVersion}:`);
for (const p of paths) console.log('  ' + p);
