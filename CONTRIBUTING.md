# Contributing to utoaudio

utoaudio is an open-source (AGPL-3.0), audiophile-grade music player.
Contributions are welcome — whether code, documentation, bug reports,
or feature requests.

## Fork & submodule workflow

utoaudio consumes three upstream libraries as **git submodules** pointing
to forks under https://github.com/utopian-society/. Changes to these
libraries should flow **fork → upstream → submodule update**, keeping
contributions modular and upstreamable.

| Submodule | Path | Fork (origin) | Upstream |
|---|---|---|---|
| Flick | `vendor/flick` | `utopian-society/Flick` | `moss-apps/Flick` |
| AMLL | `apps/desktop/src/lib/vendor/amll` | `utopian-society/applemusic-like-lyrics` | `amll-dev/applemusic-like-lyrics` |
| liquid-glass-svelte | `apps/desktop/src/lib/vendor/liquid-glass` | `utopian-society/liquid-glass-svelte` | `danilofiumi/liquid-glass-svelte` |

Each submodule has two remotes:
- **`origin`** — the utopian-society fork (where your changes land).
- **`upstream`** — the original project (where PRs are sent).

## Pulling upstream changes into a submodule

When the upstream project releases new commits you want to incorporate:

```sh
cd <submodule-path>
git fetch upstream
git checkout main
git merge upstream/main   # or rebase, if the fork policy prefers it
git push origin main      # push the merged state to the utopian-society fork
```

Then update the submodule reference in the main repo:

```sh
# from the monorepo root
git add <submodule-path>
git commit -m "chore: bump <name> submodule to upstream <sha>"
```

> **Note:** Never push directly to upstream (`moss-apps/Flick`,
> `amll-dev/applemusic-like-lyrics`, `danilofiumi/liquid-glass-svelte`).
> Changes flow through your fork (`origin`) first, then to upstream via
> a pull request.

## Contributing changes back to upstream

1. **Make your change inside the submodule.**

   ```sh
   cd <submodule-path>
   git checkout -b my-feature
   # … edit, commit …
   ```

2. **Push the branch to your fork.**

   ```sh
   git push origin my-feature
   ```

3. **Open a pull request from your fork to upstream.**

   - Go to `https://github.com/utopian-society/<repo>` on GitHub.
   - Create a PR targeting the upstream repo's `main` branch.
   - In the PR description, explain what was changed and why.

4. **Once the PR is merged upstream**, pull the merged state back into
   your fork:

   ```sh
   cd <submodule-path>
   git checkout main
   git fetch upstream
   git merge upstream/main
   git push origin main
   ```

   Then bump the submodule reference in utoaudio (see above).

## Quick submodule sync script

Run `./scripts/sync-submodules.sh` from the monorepo root to check the
state of all submodules relative to their forks and upstreams. It is
**read-only by default** — use `--pull` or `--push` for write operations.

```sh
./scripts/sync-submodules.sh          # report status only
./scripts/sync-submodules.sh --pull   # fetch + report + pull origin/main
./scripts/sync-submodules.sh --push   # fetch + report + push origin/main
```

## General contribution guidelines

- **Respect the license.** All new code in this repo is AGPL-3.0.
  Copied/derived code from upstream (Flick, AMLL) must retain attribution
  and be recorded in `THIRD_PARTY_LICENSES.md`.

- **Honour the visual identity.** Pale green (`#bef264`) + yellow
  (`#fef08a`) accents on a pure white base. Translucent liquid-glass
  aesthetic (backdrop-blur, soft edges, subtle depth shadows).

- **Keep it cross-platform.** Linux desktop is the primary dev target.
  Android is the secondary target — gate Android-specific code with
  `#[cfg(target_os = "android")]` and test that Linux builds still pass.

- **Read `progress.md` before starting.** It is the authoritative log of
  every past prompt's work, architectural decisions, and known issues.

- **Append a section to `progress.md` after your work.** Format it as:

  ```
  ## What <description> did — <one-line summary>
  ### Files created / modified
  ### Verification
  ### Architectural decisions (if any)
  ### Known issues / hand-off notes
  ```

- **Run verification before committing:**
  ```sh
  cargo build --workspace
  cargo test --workspace --exclude rust_lib_flick_player
  cd apps/desktop && pnpm run check && pnpm run build
  ```

## Code of conduct

Be kind. This is a small, focused project — every contribution matters.
Assume good intent, give constructive feedback, and keep the music
playing.