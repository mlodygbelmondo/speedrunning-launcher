# srl-auth: notes (milestone 3, stream "auth")

Microsoft login, complete and tested against a local mock of every service. **Off** until Mojang approves our own
client ID; no live login has been done (there is no client ID yet). The guide for Piotr, in Polish:
`launcher/docs/microsoft-login.md`. The API, the UI spec and what the copy may claim: the doc comment at the top of
`src/lib.rs`.

## Files

| File | What |
|---|---|
| `src/lib.rs` | Overview, API, "For the UI". |
| `src/auth.rs` | `Auth`: login, accounts, `prepare_launch`, `refresh_in_background`, `mark_in_use`. |
| `src/login.rs` | Browser + device code race, `LoginEvent`, `qr_matrix`. |
| `src/loopback.rs` | Loopback redirect listener (127.0.0.1:0, plus [::1] on the same port), state check. |
| `src/msa.rs` | Token endpoint: code exchange, refresh, device code start and polling, AADSTS mapping. |
| `src/xbox.rs` | Xbox Live, XSTS (XErr), `login_with_xbox` (403 not approved), entitlements, profile. |
| `src/tokens.rs` | Per-step token cache, `Plan`, margins (8 h launch, 12 h background, 5 min per step). |
| `src/store.rs` | `accounts.json` (no secrets), encrypted `auth/tokens/<uuid>.bin` (XChaCha20-Poly1305). |
| `src/keystore/` | Token key in Credential Manager / Keychain / Secret Service (oo7); 0600 or DPAPI file fallback. |
| `src/session.rs` | `MinecraftSession` and the exact game arguments. |
| `src/config.rs` | `AuthConfig::detect()` (`SRL_MSA_CLIENT_ID`), `Endpoints` (all configurable). |
| `src/error.rs` | `AuthError`, `XboxProblem`, `copy_key()`, `help_url()`. |
| `copy/en.json`, `copy/pl.json` | Texts to merge into the app's copy files. A test checks every error key exists. |
| `tests/chain.rs`, `tests/mock/` | 20 end-to-end tests against the mock. |
| `examples/msa-login.rs` | Terminal login for checking a client ID (guide, part C). |

## Design decisions

- **Envelope encryption** (research 1.6): Credential Manager caps a credential at 2560 bytes, so only a random
  256-bit key goes to the OS keystore, one per data folder (`key_id` in `accounts.json`, so a dev `SRL_HOME` never
  overwrites the real key). Tokens are sealed per account with the UUID as associated data.
- **Fallback:** no keystore (or `SRL_KEYSTORE=file`) puts the key in `<root>/auth/key-<id>`: 0600 in a 0700 dir on
  Unix (refused if someone loosens it), DPAPI current-user on Windows. `accounts.json` records where the key is, so a
  slow keyring at the next start is an error, not a silent switch to a file.
- **Key replacement** (fixed after review): a login makes a new key only when the old one is gone, damaged or exposed
  (`keystore::Loaded::Unusable`), and deletes what is left of it. A keystore that can't be reached right now (locked
  keyring, dismissed prompt, slow Secret Service, `SRL_KEYSTORE=file` against an `os` key, an I/O error) is
  `AuthError::Keystore` for a login too; before, it re-keyed, signed every other account out for good and could move
  the key to a file. Way out if the keyring is gone for good: log out of every account (the last one resets the key).
- **Redirect URI:** `http://localhost:<port>` (the portal accepts `http://localhost`, not `127.0.0.1`); the listener
  binds 127.0.0.1 and [::1] on the same port. If [::1] is taken on that port (browsers try ::1 first, so the redirect
  would reach the other process) it takes another port, up to 16 tries; IPv4 alone only when there is no IPv6
  loopback. `AuthConfig::redirect_host` can change it.
- **State:** checked before anything else in the redirect, required, compared in constant time. A request without our
  state gets the "not started here" page and the code is never exchanged, but the listener keeps waiting: any local
  process can reach the port, so it must not be able to end the flow. `StateMismatch` comes only at the browser
  timeout. Connections are served side by side (up to 64, 5 s read deadline each), so idle ones can't stall the
  redirect.
- **Race:** one path failing leaves the other running; a "no" (`Declined`) on either ends both. Cancelling = dropping
  the future.
- **Refresh:** restart from the latest step with 5+ minutes left. A cached step the server rejects (400/401) falls back
  once to the refresh token. The rotated refresh token is saved even when a later step fails. `invalid_grant`, a lost
  key or a different build client ID flag the account `needs_login` and delete its tokens.
- **Network down at launch:** if the saved Minecraft token still works, `prepare_launch` returns it with
  `warning: Some(Network)` (research 1.7, "launch with last token").
- **In use:** `mark_in_use` guards; the background refresh skips the account. A launch while in use needs the 8 h
  margin like any launch (fixed after review: it used to take any unexpired token without a warning); the running
  games keep their old token, which stays valid. After a passing renewal failure, launches in the next 60 s
  (`RETRY_AFTER_FAILURE`) reuse the saved token with the same warning, so a wall doesn't wait for the network once
  per instance. A launch with a cached token writes nothing (`refreshed_at` moves only on a real renewal).
- **Refresh errors:** `invalid_grant`, `interaction_required`, `consent_required` and `login_required` mean "log in
  again" (badge, tokens deleted); `temporarily_unavailable` is passing (reported as a 503 `Service`).
- **`--clientId`:** an anonymous per-installation ID (`install_id` in `accounts.json`), like the official launcher's;
  not the Entra ID. `--xuid` comes from the `xuid` claim of the Minecraft token (JWT, signature not checked).
- **Other launchers' IDs** are refused by `AuthConfig`: Pandora's (read in its source), Prism's and MultiMC's (from
  memory, not checked against their source).

## Integration after M2 (not done here: srl-game and srl-app were off limits)

1. **srl-game:** `LaunchOptions.account` takes either the offline account or a `MinecraftSession`. For a session,
   fill the placeholder map from `session.placeholders()` (replaces `auth_*`, `user_type`, `clientid`,
   `auth_xuid`, `user_properties`); table in `src/session.rs`. Mask the token in any logged or shown command line
   with `session.redact()`.
2. **srl-app:** create `Auth::new(&paths, AuthConfig::detect())` on the backend runtime; at start
   `refresh_in_background()`; on Play with a Microsoft account `prepare_launch(uuid)` then hold
   `mark_in_use(uuid)` until the game exits; account page per `lib.rs` "For the UI"; merge `copy/*.json`.
3. **CI:** pass `SRL_MSA_CLIENT_ID` from a repo variable to the build once approved (guide, part E). Tests must keep
   using the file keystore (they do: `KeystoreChoice::FileOnly`).

## srl-core changes

None. srl-auth uses `Paths::root()` and `USER_AGENT` only. The workspace change is one entry in `members`.

## Verification (what I ran, 2026-10-26)

All through `srl-cpu`, `CARGO_TARGET_DIR=~/.cache/srl-target/m3-auth`, `-j 8`.

- `cargo test -p srl-auth`: 32 unit tests and 16 mock-chain tests pass (Linux). They cover:
  - the RFC 7636 appendix B PKCE vector;
  - a state mismatch (and a missing state) rejected, with the code never exchanged;
  - device code `authorization_pending`, `slow_down` (+5 s, sticky, measured), `expired_token`, local expiry and
    `authorization_declined`;
  - every XSTS XErr code;
  - 403 "Invalid app registration", 429 with `Retry-After`, profile 404 with no / purchase / Game Pass entitlements;
  - the 8 h / 12 h / 5 min margins, step-wise renewal and refresh token rotation;
  - `invalid_grant` leading to a new login, a server-revoked cached step, offline launch with the last token, an
    account in use, a changed client ID, logout;
  - no token in any file on disk or in any `Debug` output.
- `cargo clippy -p srl-auth --all-targets -D warnings`, Linux and `--target x86_64-pc-windows-gnu`: clean.
  `cargo clippy --workspace --all-targets --locked -D warnings`: clean.
- File fallback, headless Linux: `SRL_AUTH_OS_TEST=absent DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/bus cargo
  test -p srl-auth keystore -- --ignored`: Auto mode fell back to the file in under 1 ms.
- Secret Service through oo7: the same ignored test with `SRL_AUTH_OS_TEST=present`, inside `dbus-run-session` with a
  throwaway `gnome-keyring-daemon --unlock` and private HOME/XDG dirs (never the desktop's keyring): set, replace,
  get, delete, and `Keystore::create` chose `Os`. The test daemon was gone afterwards.
- Windows: the test binaries cross-built for `x86_64-pc-windows-gnu` and run under Wine (throwaway prefix, inside
  `xvfb-run`): 32 unit tests (the Windows set: a DPAPI round trip instead of the Unix permission test), the 16
  chain tests, and the ignored Credential Manager round trip (`SRL_AUTH_OS_TEST=present`) pass.
  Wine is not Windows: CI's `windows-latest` will run the non-ignored tests on real Windows on the next push.
- Live, against the real Microsoft endpoint, with a random unregistered GUID and `--no-browser`
  (`examples/msa-login`): the device code request came back AADSTS700016 and was reported as
  `auth.error.app_misconfigured` ("the client ID is unknown to Microsoft"). Nothing else live: a real login needs our
  client ID.

## Review fixes (branch m3-auth-fix, 2026-10-26)

Each finding was checked first. The four new chain tests fail with cb2795f's `src/` (in-use margin, refresh errors,
offline wall, cached launch); `a_keystore_error_during_a_login_keeps_the_old_key` fails with the old "swallow the
error and re-key" branch put back. All pass now.

| Finding | Verdict | Fix | Test |
|---|---|---|---|
| high + medium: keystore error at login re-keys | real | `keystore::Loaded`; re-key only on gone/damaged/exposed, else `Keystore` error | `store::a_keystore_error_during_a_login_keeps_the_old_key`, `an_exposed_key_file_is_replaced_by_a_login`, ignored `auto_mode_without_a_keystore_never_replaces_an_os_key` |
| medium: in-use launch skips the 8 h margin | real | in-use shortcut removed; 60 s memo of a passing failure | chain `a_launch_while_in_use_still_needs_the_launch_margin`, `a_failed_renewal_is_not_retried_for_every_instance` |
| medium: refresh `interaction_required` etc. | real | `msa::refresh_error`; `temporarily_unavailable` transient (also keeps device polling) | `msa::refresh_errors_that_need_the_player_ask_for_a_new_login`, chain `refresh_errors_that_need_the_player_or_just_time` |
| medium: guide part C | real (authorize with a random GUID: 302 to login.live.com, 200 page with `unauthorized_client`, no redirect) | part C runs twice: browser only (`SignedIn(Browser)`), then `--no-browser` | - |
| low: [::1] squatted | real (by reading: the v6 bind error was dropped) | retry another port on AddrInUse/PermissionDenied, v4-only only without IPv6 | `loopback::a_port_taken_on_ipv6_is_not_used` |
| low: probes end or stall the browser path | real (the old unit test asserted that a mismatch ends it) | mismatch keeps listening, reported at timeout; JoinSet, 64 connections | `a_foreign_state_is_refused_and_the_real_redirect_still_works`, `only_foreign_states_end_as_a_mismatch_at_the_timeout`, `idle_connections_dont_delay_the_redirect` |
| low: cached launch moves `refreshed_at` | real | `renew` returns `Renewal::{Cached, Renewed}`; cached writes nothing | chain `a_launch_with_a_cached_token_changes_nothing_on_disk` |
| low: Declined says "in the browser" | real | "The sign-in was declined." (en, pl, `Display`) | copy test |

Behaviour changes for the UI: `AuthError::Keystore` can now come from a login (say "unlock your keyring and try
again"); `StateMismatch` arrives only at the browser timeout; the `keystore_file_note` copy no longer claims "no
keyring found".

Verification of the fixes (all through `srl-cpu`, `CARGO_TARGET_DIR=~/.cache/srl-target/m3-auth`):

- `cargo test -p srl-auth` (Linux): 38 unit (3 ignored) + 20 chain tests pass.
- `SRL_AUTH_OS_TEST=absent DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/bus ... -- --ignored`: Auto mode with an
  `os` key and no Secret Service: the login write failed with `Keystore` in 0.2 ms, no key file, no badge; a fresh
  key still falls back to the file.
- oo7 against a throwaway gnome-keyring (`dbus-run-session`, `env -i`, private HOME/XDG, throwaway password, no
  DISPLAY): `os_keystore_round_trip` passes. An empty password made the daemon want a prompt, which failed for lack
  of a display (nothing reached the desktop). Dirs deleted, the daemon killed.
- Windows-gnu binaries under Wine (throwaway prefix, `xvfb-run`, no WAYLAND_DISPLAY): 36 unit (the Windows set, 3 ignored), 20 chain
  and the ignored Credential Manager round trip pass. Prefix deleted.
- clippy `-D warnings`: srl-auth Linux and `x86_64-pc-windows-gnu`, workspace `--locked`; `cargo fmt --all --check`.
- Live: one GET of the real authorize endpoint with a random GUID (see the table). No login.

## Open issues

- **No live login yet.** Needs Piotr's Entra registration and Mojang's approval (guide). The Xbox/XSTS/Minecraft
  shapes follow research, Pandora and minecraft.wiki; only the mock has answered them.
- **Offline accounts vs Mojang's policy:** the form rejects apps that bypass authentication; UN_Nexo was rejected
  for offline play. Piotr must decide before submitting (guide, part D).
- **macOS is not compiled:** `cargo check --target aarch64-apple-darwin` stops at `ring`'s C build (no Apple
  toolchain here). `keystore/macos.rs` was checked against the security-framework 3.7 source by reading only.
- Game Pass entitlement names (`*game_pass*`) are from community docs, unverified.
- 429 is reported with `retry_after`, not retried automatically. The 60 s failure memo ignores `retry_after`.
- A permanently gone keyring (with accounts whose key was there) blocks new logins with `Keystore` until the player
  logs out of every account; the copy says so. A "reset saved sign-ins" button could do it in one step later.
- The store lock is held during a refresh's HTTP calls (30 s timeout each), so `set_active`, `remove` and
  `prepare_launch` wait for a running refresh (on purpose for `prepare_launch`: no double refresh). `accounts()`
  reads without the lock.
- On Linux the first login may show the keyring's own unlock prompt (that is the Secret Service working).
- `Cargo.lock` gained crates (chacha20poly1305, qrcode, ...); on merge with `launcher`, take theirs and rebuild.
