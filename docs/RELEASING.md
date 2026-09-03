# Releasing

There is no automated version bump — the three version files
(`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`) are edited by hand and
must agree. `release.yml`'s "Verify tag matches the configured version" step catches a
mismatch at tag time (added for issue #25, after `v0.2.3` shipped `v0.2.2`'s binaries because
this was missed), but it can't catch it before you push the tag.

## One-time setup: updater signing keys

Skip this section if the keys already exist — check `gh secret list` for
`TAURI_SIGNING_PRIVATE_KEY`.

1. Generate a keypair:
   ```bash
   npm run tauri signer generate -- -w ~/.tauri/click.key
   ```
   This prints a public key and writes the private key (optionally password-protected) to
   `~/.tauri/click.key`. **Store the private key and its password somewhere durable outside
   this repo** — losing them permanently breaks the update path for every existing install
   (see `SECURITY.md`'s "Update signing" section).
2. Add two repo secrets (Settings → Secrets and variables → Actions, or `gh secret set`):
   - `TAURI_SIGNING_PRIVATE_KEY` — the contents of the private key file.
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — its password (empty string if you didn't set one).
3. Paste the printed **public** key into `src-tauri/tauri.conf.json`'s
   `plugins.updater.pubkey`, replacing `REPLACE_WITH_GENERATED_PUBLIC_KEY`, and commit it.
   The public key is not a secret — it's meant to ship inside the app.

Until all three of these are done, the updater is inert: `check_for_updates` reports
`unavailable` (no pubkey configured yet) rather than failing loudly, and every other build
(`npm run tauri dev`, `npm run tauri build`, `smoke.yml`) is unaffected either way.

## Cutting a release

1. Bump all three version files to the same version.
2. Update `CHANGELOG.md`: rename the `## [Unreleased]` heading to the new version and date,
   and start a fresh empty `## [Unreleased]` above it.
3. Commit, then tag and push:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
4. `release.yml` builds the NSIS and MSI installers, signs the updater artifacts (using the
   secrets above), generates `latest.json`, and creates a **draft** GitHub Release with all of
   it attached.
5. **Publish the draft release.** This is the step that actually matters for the updater —
   `plugins.updater.endpoints` points at `releases/latest/download/latest.json`, which only
   ever resolves against the latest **published** (non-draft) release. A draft is invisible to
   every installed copy of Click.

## Verifying an update actually works

Existing installs have no updater and can never auto-update to anything — only installs of the
first updater-enabled release onward benefit. Before trusting a real release to the updater,
it's worth proving the whole loop once against a throwaway tag:

1. Cut and publish a real tagged release as above (a prerelease works fine for this).
2. Install an *older* build.
3. Trigger a check (tray → "Check for updates…", or the diagnostics footer's button) and
   confirm it offers the new version, installs, and relaunches into it.
