# SolidCraft MM: personal builds

This branch (`mm/tooling`) holds only the tooling that builds my own SolidCraft from upstream
`main` plus my open pull requests. It shares no history with SolidCraft and is never merged
anywhere. It is the fork's default branch so the scheduled workflow runs.

## Branches

| Branch | What it is | Who writes it |
|---|---|---|
| `main` | an exact mirror of `tangency-labs/solidcraft` `main` (fast-forward only) | the workflow |
| `feature/*`, `bugfix/*` | my pull request branches, as usual | me |
| `mm/tooling` | this tooling; the fork's default branch | me |
| `mm/releases` | upstream `main` + my open pull requests, rebuilt from scratch each run | the workflow (force-pushed) |
| `mm/resolve/<branch>` | optional: a stored conflict fix for `<branch>` | me, only when needed |

Everything of mine is under `mm/`, a prefix upstream does not use. Upstream's release workflow
builds on a push to `release`, and upstream keeps `release-*` branches, so neither name is used
here. Tags are `mm-*`; upstream tags are `v*`.

## Which branches are merged

Every open pull request I have in `tangency-labs/solidcraft` from this fork, lowest number first.
New pull requests are picked up on the next run and merged ones drop out. `mm/branches.txt` adds
branches without a pull request (`include <branch>`) or leaves one out (`exclude <branch>`).

## Names

| | Format | Example |
|---|---|---|
| Version | `<upstream version>+mm.<UTC date>.<commit>` | `0.40.0-rc.2+mm.20261105.ec4d018` |
| App | `SolidCraft MM.app`, bundle id `io.github.martinmajewski.solidcraft` | |
| DMG | `solidcraft-mm-<version>-macos-<arch>.dmg` | `solidcraft-mm-0.40.0-rc.2+mm.20261105.ec4d018-macos-aarch64.dmg` |
| Release tag | `mm-<upstream version>-<date>-<commit>` | `mm-0.40.0-rc.2-20261105-ec4d018` |

`<commit>` is the `mm/releases` commit. `+mm…` is semver build metadata, which version
comparisons ignore: the app's update check still compares against official releases correctly
and will report a newer stable upstream release. The app has its own name and bundle id, so it
installs next to an official `SolidCraft.app`; settings and recent files are probably shared.

## On GitHub (automatic, free)

`.github/workflows/mm-release-build.yml` runs daily at 04:20 UTC and from **Actions → MM release
build → Run workflow**. It syncs `main`, rebuilds `mm/releases`, and when that changed (or was
never built) builds the macOS app on a `macos-15` runner and publishes it as a prerelease.
Public repositories pay nothing for GitHub-hosted runners.

One-time setup:

1. Create a fine-grained personal access token at
   <https://github.com/settings/personal-access-tokens/new>: resource owner MartinMajewski,
   *Only select repositories* → `solidcraft`, permissions **Contents: Read and write** and
   **Workflows: Read and write**. A 1-year expiry is fine; renew it when it runs out.
2. Store it as a repository secret named `MM_SYNC_TOKEN`:
   `gh secret set MM_SYNC_TOKEN -R MartinMajewski/solidcraft` (paste the token when asked).
3. Done earlier: the default branch is `mm/tooling`, and the upstream workflows that came with
   the fork are disabled (`gh workflow list -R MartinMajewski/solidcraft --all`), so syncing
   `main` does not start upstream's CI and release pipeline here.

Install a build: download the DMG from the fork's Releases, drag the app to Applications, then
once (the app is ad-hoc signed, not notarized):

    xattr -dr com.apple.quarantine "/Applications/SolidCraft MM.app"

## On this Mac

    mm/local-release.sh                 # sync locally, build, install /Applications/SolidCraft MM.app
    mm/local-release.sh --push          # ... and update the fork's main and mm/releases
    mm/local-release.sh --prefer-local  # use local branches, to try commits not pushed yet

It builds in its own worktree (`../solidcraft-mm-build`), so the checkout you work in is never
switched. The parts can run alone: `mm/sync.sh --help`, `mm/build-macos.sh --help`.

## When a merge conflicts

The run fails and its summary names the branch and the files. Then:

    git switch -c mm/resolve/<branch> fork/<branch>
    git merge upstream/main          # and any earlier branch in the list it conflicts with
    # resolve, commit
    git push fork mm/resolve/<branch>

and run the workflow again. `mm/resolve/<branch>` is merged right before `<branch>`; delete it
once the pull request is merged. Locally, git's rerere cache also remembers resolutions.
