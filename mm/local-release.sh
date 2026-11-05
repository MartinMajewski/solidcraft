#!/usr/bin/env bash
# The whole thing on this Mac: rebuild mm/releases, build it in its own worktree, install the app.
#
#   mm/sync.sh            -> the mm/releases commit (pushed only with --push)
#   <build worktree>      a detached worktree at that commit (default: ../solidcraft-mm-build next
#                         to the repo), so your own checkout and its branch are never switched
#   mm/build-macos.sh     -> dist/mm/*.dmg in the build worktree, and "/Applications/SolidCraft MM.app"
#
# Usage: mm/local-release.sh [--repo DIR] [--build-dir DIR] [--push] [--prefer-local] [--no-install]
#   --repo DIR       your SolidCraft clone with the remotes `upstream` and `fork`
#                    (default: ../solidcraft next to this tooling checkout)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

REPO="$HERE/../../solidcraft"
BUILD=
SYNC_ARGS=()
INSTALL=(--install)
while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift 2 ;;
    --build-dir) BUILD="$2"; shift 2 ;;
    --push | --prefer-local) SYNC_ARGS+=("$1"); shift ;;
    --no-install) INSTALL=(); shift ;;
    -h | --help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
REPO="$(cd "$REPO" && git rev-parse --show-toplevel)"
BUILD="${BUILD:-$(dirname "$REPO")/solidcraft-mm-build}"
OUT="$(mktemp -d "${TMPDIR:-/tmp}/mm-sync.XXXXXX")"

SHA="$("$HERE/sync.sh" --repo "$REPO" --out "$OUT" ${SYNC_ARGS[@]+"${SYNC_ARGS[@]}"})"
cat "$OUT/manifest.md"

if [ -d "$BUILD" ]; then
  if [ -n "$(git -C "$BUILD" status --porcelain --untracked-files=no)" ]; then
    echo "$BUILD has local changes; not switching it" >&2; exit 1
  fi
  git -C "$BUILD" checkout --quiet --detach "$SHA"
else
  git -C "$REPO" worktree add --quiet --detach "$BUILD" "$SHA"
fi
mkdir -p "$BUILD/dist/mm"
cp "$OUT/manifest.md" "$BUILD/dist/mm/manifest.md"
"$HERE/build-macos.sh" --src "$BUILD" ${INSTALL[@]+"${INSTALL[@]}"}
