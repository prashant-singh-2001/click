# Security Policy

## Supported versions

Click is pre-1.0 and under active development. Only the latest release receives security fixes.

| Version | Supported |
|---|---|
| 0.2.x | ✅ |
| < 0.2 | ❌ |

## Reporting a vulnerability

**Please do not open a public issue for security vulnerabilities.**

Report privately to **ps47600@gmail.com** with:

- a description of the issue and its impact,
- steps to reproduce (a proof of concept if you have one),
- affected version and OS.

You'll get an acknowledgement, and we'll work with you on a fix and coordinated disclosure. As a small personal project there is no bounty, but your contribution will be credited if you'd like.

## Security model — please read

Click's whole purpose is to **launch applications** and (in future releases) **run commands**. That means **a workspace configuration is executable content**: opening and launching one runs whatever it points at, with your user privileges.

Consequences to be aware of:

- **Only launch workspaces you created or fully trust.** Treat a `workspaces.json` from someone else the way you'd treat a shell script from them.
- **Import/export and command actions are deliberately not shipped yet.** They are the point at which an untrusted config becomes dangerous. Per the design spec (`docs/REQUIREMENTS.md`, §7), when either lands it **must** ship together with:
  - **SEC-1** — importing a config shows exactly what it will run and requires explicit confirmation before the first launch; imported configs are never executed silently.
  - **SEC-2** — a dry-run/preview that lists every command, path, and argument without executing anything.
- **Secrets** should not be stored in plaintext config. Prefer `${VAR}` variables that resolve from your environment over hard-coding credentials into a workspace.
- **The local log file** (`%LOCALAPPDATA%\com.launchpad.app\logs\click.log`) records the target path of every action launched, but not resolved command-line arguments or working directories — those can carry a `${VAR}`-resolved secret, so they're only written when you explicitly opt in via `CLICK_LOG=debug`. Keep that in mind before attaching a debug-level log to a bug report.

## Unsigned builds

Release binaries are **not currently code-signed** (Authenticode), so Windows SmartScreen will warn on first run. This is expected for now, but it means you should only run installers you obtained from the official [Releases](../../releases) page. Code signing (SEC-5) is still on the roadmap.

This is a **separate concern from update signing** below — Authenticode is about trusting where an installer came from; the updater's signature is about trusting that an automatic update wasn't tampered with in transit. Adding the updater does not add Authenticode signing, and vice versa.

## Update signing

Click checks for updates (issue #25) against this repo's GitHub Releases and verifies every downloaded update against a public key baked into the app before installing it. The trust model:

- The **private** signing key lives outside this repository, held by the maintainer, and is stored as GitHub Actions secrets (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) used only by the release workflow. It is never committed and never appears in any build log.
- The **public** key is committed in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`) and ships inside every build — that's intentional; a public key is not a secret.
- **If the private key is ever lost**, every existing install's update path is permanently broken — there is no way to sign a new release those installs will trust. Recovery means generating a new keypair, shipping it in a new release, and every user reinstalling manually once (the same manual-reinstall flow this feature exists to remove). There is no rotation mechanism today.
- Because updates run with your full user privileges the moment you approve them, the private key is exactly as sensitive as commit access to this repository's release workflow — treat a compromise of one as a compromise of both.
