# Key art and fonts

NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Renders

Every image in `hero/` and `card/` is our own render: Blender 5.2.3, Cycles, OpenImageDenoise and AgX view transform.

- **Scenes:** voxel scenes built in code.
- **Textures:** our own procedural 16×16 textures, generated with numpy/PIL. No file from the Minecraft jar is used.
- **Structures:** where a scene shows a real structure (bastion, fortress, stronghold, mode scenes), its block layout
  was read from a world we generated ourselves with the official dedicated server. It is then rebuilt with our own
  textures. Ranked (`hero/ranked.jpg`, `card/ranked.jpg`) is the plains village that starts at (−816, 32) in Java
  1.16.1, seed 2026, seen from (−870, 84, 95); one cave mouth by its pond is filled in.
- **Sources:** everything lives in `design-proposals/2026-10-25-redesign/`. The scripts are in `art-src/` and the
  1920×1080 masters in `art/masters/`. Its README lists the seed and coordinates of every structure.
- **Sizes:** `tools/make-art.sh ../design-proposals/2026-10-25-redesign/art/masters` writes the two shipped sizes.

## Fonts (`assets/fonts/`)

| Font | Licence | File |
|---|---|---|
| Archivo, as static instances at width 112, renamed "Archivo SRL" so they are not confused with the original variable font | SIL OFL 1.1 | `ArchivoSRL-OFL.txt` |
| IBM Plex Mono, unmodified original files (Reserved Font Name "Plex") | SIL OFL 1.1 | `IBMPlexMono-OFL.txt` |
| Inter | SIL OFL 1.1 | `Inter-LICENSE.txt` |
| JetBrains Mono (v2 screens) | SIL OFL 1.1 | `JetBrainsMono-OFL.txt` |
