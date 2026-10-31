# Third-party notices

Code adapted from other projects, with its licence. Add an entry whenever you adapt code.

## Pandora Launcher

<https://github.com/Moulberry/PandoraLauncher>, commit 2d83d6f.

```text
MIT License

Copyright (c) 2025 Moulberry

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Files adapted from it:

- `crates/srl-game/src/schema.rs`: version JSON, version manifest, asset index and Java runtime
  types, from `crates/schema/src/version.rs`, `version_manifest.rs`, `assets_index.rs`,
  `java_runtimes.rs` and `java_runtime_component.rs`.
- `crates/srl-game/src/rules.rs`: rule evaluation, from `crates/backend/src/launch/mod.rs`
  (`LaunchRuleContext::check_rules`, `check_rule`).
- `crates/srl-game/src/libraries.rs`: library de-duplication, natives lookup and profile merging,
  from `crates/backend/src/launch/mod.rs` (`collect_libraries`) and `crates/schema/src/version.rs`
  (`PartialMinecraftVersion::apply_to`).
- `crates/srl-game/src/java.rs`: Mojang Java runtime install, from `crates/backend/src/launch/mod.rs`
  (`load_mojang_java_binary`, `do_java_runtime_load`).
- `crates/srl-game/src/launch.rs`: argument rules and `${placeholder}` substitution, from
  `crates/backend/src/launch/mod.rs` (`LaunchContext::launch`, `process_arguments`,
  `expand_argument`, `resolve_expansion`).
- `crates/srl-practice/src/level.rs`: reading a world's `level.dat` (gunzip, the named root
  compound, then `Data` and `LevelName`), from `crates/backend/src/instance.rs`
  (`load_world_summary`), with the nesting-depth limit of `crates/nbt/src/decode.rs`. The NBT
  reader itself is our own and keeps only the fields we need.
- `crates/srl-auth/src/xbox.rs`: Xbox Live, XSTS and Minecraft login requests and responses, and
  the clock-skew handling of token expiry, from `crates/auth/src/authenticator.rs` and `models.rs`.
- `crates/srl-auth/src/msa.rs`: the shape of the MSA code exchange and refresh, from
  `crates/auth/src/authenticator.rs` (rewritten without the `oauth2` crate).
- `crates/srl-auth/src/loopback.rs`: the loopback redirect listener, from
  `crates/auth/src/serve_redirect.rs`.
- `crates/srl-auth/src/keystore/linux.rs`, `macos.rs`, `windows.rs`: Secret Service (oo7),
  Keychain and Credential Manager access, from `crates/auth/src/secret.rs`.

## EasyInjectBundled

<https://github.com/jojoe77777/EasyInjectBundled>, commit f59501e: the installer and injector that
ships Toolscreen.

```text
MIT License (MIT)

Copyright (c) 2026 jojoe77777

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Files adapted from it:

- `crates/srl-tools/src/win.rs`: DLL injection (`inject`), from
  `src/main/java/com/easyinject/DllInjector.java` (`injectDllToProcessWithResult`), with the order
  from `Main.java` (`runWatcherMode`: liblogger first, then Toolscreen.dll).

## Read for behaviour only

No code taken from these:

- Toolscreen (MIT, <https://github.com/jojoe77777/Toolscreen>, commit 92c7e9b): where it keeps its
  settings and how its DLL hooks the game.
- GPL-3.0 projects: MCSR Launcher (how it runs Toolscreen before the game), waywall (`waywall wrap`,
  its environment and signal handling, where its packages put the patched GLFW) and Ninjabrain
  Bot (its window title and shutdown hook).
