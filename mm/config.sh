# shellcheck shell=bash
# Shared settings for the mm/ release tooling. Source it; every value can be overridden from the
# environment.
#
# Every branch this tooling writes lives under mm/, a namespace upstream does not use. Upstream's
# own release machinery reacts to `release` and `release-*` (release.yml builds on a push to
# `release`), so nothing here is called that.

UPSTREAM_REPO="${UPSTREAM_REPO:-tangency-labs/solidcraft}"
FORK_REPO="${FORK_REPO:-MartinMajewski/solidcraft}"
# Open pull requests by this author in UPSTREAM_REPO, from FORK_REPO, are merged into the build.
PR_AUTHOR="${PR_AUTHOR:-MartinMajewski}"

# Remote names in the local clone (CI uses `origin` for the fork).
UPSTREAM_REMOTE="${UPSTREAM_REMOTE:-upstream}"
FORK_REMOTE="${FORK_REMOTE:-fork}"

# Generated integration branch: upstream main + the feature branches. Rebuilt on every run.
RELEASE_BRANCH="${RELEASE_BRANCH:-mm/releases}"
# Hand-made conflict resolutions: mm/resolve/<feature branch> is merged right before that branch.
RESOLVE_PREFIX="${RESOLVE_PREFIX:-mm/resolve/}"

# Build identity. The label goes into the version's semver build metadata
# (0.40.0-rc.2+mm.20261105.ec4d018), which version comparisons ignore.
BUILD_LABEL="${BUILD_LABEL:-mm}"
APP_NAME="${APP_NAME:-SolidCraft MM}"
BUNDLE_ID="${BUNDLE_ID:-io.github.martinmajewski.solidcraft}"

# Identity and fixed dates for the merge commits, so identical inputs give an identical SHA.
BOT_NAME="${BOT_NAME:-SolidCraft MM release build}"
BOT_EMAIL="${BOT_EMAIL:-MartinMajewski@users.noreply.github.com}"

FORK_OWNER="${FORK_REPO%%/*}"
