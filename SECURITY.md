# Security Policy

## Supported versions

Click is pre-1.0 and under active development. Only the latest release receives security fixes.

| Version | Supported |
|---|---|
| 0.1.x | ✅ |
| < 0.1 | ❌ |

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

## Unsigned builds

Release binaries are **not currently code-signed**, so Windows SmartScreen will warn on first run. This is expected for now, but it means you should only run installers you obtained from the official [Releases](../../releases) page. Code signing is on the roadmap.
