#!/usr/bin/env bash
# Rebuild the mm/releases integration branch:
#
#   1. fast-forward the fork's main to upstream main (never forced; a diverged main is left alone)
#   2. start from upstream main and merge, lowest pull request number first, every open pull
#      request PR_AUTHOR has in UPSTREAM_REPO from FORK_REPO, then the `include` lines of
#      branches.txt; `exclude` lines drop a branch
#   3. before a branch, merge mm/resolve/<branch> when the fork has it (a stored conflict fix)
#   4. push the result to mm/releases when it changed (--force-with-lease: it is a generated branch)
#
# The merges happen in a temporary worktree, so the checkout you run this from is not touched.
# The merge commits use a fixed identity and upstream main's commit date, so the same inputs give
# the same SHA: an unchanged result is detected by comparing SHAs.
#
# Usage: mm/sync.sh [--repo DIR] [--out DIR] [--push] [--prefer-local]
#   --repo DIR       the clone to work in (default: the current directory); it needs the remotes
#                    UPSTREAM_REMOTE and FORK_REMOTE (see config.sh)
#   --out DIR        where manifest.md, sha and (on a conflict) conflict.md go (default: a temp dir)
#   --push           update the fork's main and mm/releases; without it nothing leaves this machine
#   --prefer-local   use a local branch instead of the fork's copy when it exists (to try commits
#                    you have not pushed yet)
#
# Exit status: 0 done, 1 a merge conflict (see conflict.md), 2 bad usage or setup.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=config.sh
. "$HERE/config.sh"

REPO=.
OUT=
PUSH=0
PREFER_LOCAL=0
while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --push) PUSH=1; shift ;;
    --prefer-local) PREFER_LOCAL=1; shift ;;
    -h | --help) sed -n '2,25p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
REPO="$(cd "$REPO" && git rev-parse --show-toplevel)"
if [ -z "$OUT" ]; then OUT="$(mktemp -d "${TMPDIR:-/tmp}/mm-sync.XXXXXX")"; fi
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
rm -f "$OUT/manifest.md" "$OUT/sha" "$OUT/conflict.md"

g() { git -C "$REPO" "$@"; }
log() { printf '==> %s\n' "$*" >&2; }
command -v gh >/dev/null || { echo "gh (GitHub CLI) is required" >&2; exit 2; }
for r in "$UPSTREAM_REMOTE" "$FORK_REMOTE"; do
  g remote get-url "$r" >/dev/null 2>&1 || { echo "remote '$r' is missing in $REPO" >&2; exit 2; }
done

log "fetching $UPSTREAM_REMOTE main and the fork's branches"
g fetch --quiet --no-tags "$UPSTREAM_REMOTE" "+refs/heads/main:refs/remotes/$UPSTREAM_REMOTE/main"
g fetch --quiet --no-tags --prune "$FORK_REMOTE" "+refs/heads/*:refs/remotes/$FORK_REMOTE/*"
UP="$(g rev-parse "refs/remotes/$UPSTREAM_REMOTE/main")"

# ---- 1. the fork's main -------------------------------------------------------------------------
FORK_MAIN="$(g rev-parse -q --verify "refs/remotes/$FORK_REMOTE/main" || true)"
if [ "$FORK_MAIN" = "$UP" ]; then
  log "fork main is up to date"
elif [ -n "$FORK_MAIN" ] && ! g merge-base --is-ancestor "$FORK_MAIN" "$UP"; then
  log "WARNING: fork main has commits upstream does not have; leaving it as it is"
elif [ "$PUSH" = 1 ]; then
  log "fast-forwarding fork main to ${UP:0:10}"
  g push --quiet "$FORK_REMOTE" "$UP:refs/heads/main"
else
  log "fork main is behind upstream (not pushed: no --push)"
fi

# ---- 2. which branches --------------------------------------------------------------------------
# Lines: "<pr number or ->\t<branch>\t<title>"
LIST="$OUT/branches.tsv"
gh pr list -R "$UPSTREAM_REPO" --author "$PR_AUTHOR" --state open --limit 200 \
  --json number,headRefName,headRepositoryOwner,title \
  --jq ".[] | select(.headRepositoryOwner.login == \"$FORK_OWNER\") | [.number, .headRefName, .title] | @tsv" |
  sort -n >"$LIST"
EXCLUDES=" "
if [ -f "$HERE/branches.txt" ]; then
  while read -r verb branch _; do
    case "$verb" in
      include) if ! cut -f2 "$LIST" | grep -qxF "$branch"; then printf -- '-\t%s\t%s\n' "$branch" "(no pull request)" >>"$LIST"; fi ;;
      exclude) EXCLUDES="$EXCLUDES$branch " ;;
      '' | '#'*) ;;
      *) echo "branches.txt: unknown line '$verb $branch'" >&2; exit 2 ;;
    esac
  done <"$HERE/branches.txt"
fi

# ---- 3. merge -----------------------------------------------------------------------------------
WT="$(mktemp -d "${TMPDIR:-/tmp}/mm-releases.XXXXXX")"
cleanup() { g worktree remove --force "$WT" >/dev/null 2>&1 || rm -rf "$WT"; g worktree prune; }
trap cleanup EXIT
g worktree add --quiet --detach "$WT" "$UP"
w() { git -C "$WT" -c rerere.enabled=true -c rerere.autoupdate=true "$@"; }

GIT_AUTHOR_DATE="$(g show -s --format=%cI "$UP")"
export GIT_AUTHOR_NAME="$BOT_NAME" GIT_AUTHOR_EMAIL="$BOT_EMAIL" GIT_AUTHOR_DATE
export GIT_COMMITTER_NAME="$BOT_NAME" GIT_COMMITTER_EMAIL="$BOT_EMAIL" GIT_COMMITTER_DATE="$GIT_AUTHOR_DATE"

branch_ref() { # branch_ref <branch>: the ref to merge, or nothing
  if [ "$PREFER_LOCAL" = 1 ] && g rev-parse -q --verify "refs/heads/$1^{commit}" >/dev/null; then
    echo "refs/heads/$1"
  elif g rev-parse -q --verify "refs/remotes/$FORK_REMOTE/$1^{commit}" >/dev/null; then
    echo "refs/remotes/$FORK_REMOTE/$1"
  fi
}

# merge_one <ref> <message> <branch>: merge; on a conflict rerere may already know the fix.
merge_one() {
  if w merge --quiet --no-ff --no-edit -m "$2" "$1" >"$OUT/merge.log" 2>&1; then return 0; fi
  if [ -z "$(w diff --name-only --diff-filter=U)" ] && w commit --quiet --no-edit >>"$OUT/merge.log" 2>&1; then
    log "  conflicts resolved from the rerere cache"
    return 0
  fi
  {
    echo "# mm/releases: merging \`$3\` conflicts"
    echo
    echo "Merged before it: upstream main \`${UP:0:10}\`$(merged_so_far)."
    echo
    echo "Conflicting files:"
    echo
    w diff --name-only --diff-filter=U | sed 's/^/- /'
    echo
    echo "Fix: create \`$RESOLVE_PREFIX$3\` from \`$3\`, merge upstream main (and any branch listed"
    echo "above that it conflicts with) into it, resolve, push it to the fork, run the build again."
  } >"$OUT/conflict.md"
  w merge --abort >/dev/null 2>&1 || true
  cat "$OUT/conflict.md" >&2
  exit 1
}
MERGED=""
merged_so_far() { if [ -n "$MERGED" ]; then printf ', then %s' "$MERGED"; fi; }

{
  echo "Upstream main: \`${UP}\` ($UPSTREAM_REPO)"
  echo
  echo "Merged:"
  echo
} >"$OUT/manifest.md"
COUNT=0
while IFS="$(printf '\t')" read -r num branch title; do
  case "$EXCLUDES" in *" $branch "*) log "skipping $branch (excluded)"; continue ;; esac
  ref="$(branch_ref "$branch")"
  if [ -z "$ref" ]; then log "WARNING: $branch is not on the fork; skipped"; continue; fi
  label="$branch"; [ "$num" != "-" ] && label="$branch (PR #$num)"
  resolve="$(branch_ref "$RESOLVE_PREFIX$branch")"
  if [ -n "$resolve" ]; then
    log "merging $RESOLVE_PREFIX$branch"
    merge_one "$resolve" "$RELEASE_BRANCH: merge $RESOLVE_PREFIX$branch" "$RESOLVE_PREFIX$branch"
  fi
  log "merging $label"
  merge_one "$ref" "$RELEASE_BRANCH: merge $label" "$branch"
  tip="$(g rev-parse "$ref")"
  pr=""; [ "$num" != "-" ] && pr="[#$num](https://github.com/$UPSTREAM_REPO/pull/$num) "
  echo "- $pr\`$branch\` at \`${tip:0:10}\`: $title" >>"$OUT/manifest.md"
  MERGED="${MERGED:+$MERGED, }\`$branch\`"
  COUNT=$((COUNT + 1))
done <"$LIST"
[ "$COUNT" = 0 ] && echo "- (nothing: upstream main only)" >>"$OUT/manifest.md"

NEW="$(w rev-parse HEAD)"
OLD="$(g rev-parse -q --verify "refs/remotes/$FORK_REMOTE/$RELEASE_BRANCH" || true)"
printf '\n%s: `%s`\n' "$RELEASE_BRANCH" "$NEW" >>"$OUT/manifest.md"
echo "$NEW" >"$OUT/sha"

# ---- 4. push ------------------------------------------------------------------------------------
CHANGED=true
if [ "$NEW" = "$OLD" ]; then
  CHANGED=false
  log "$RELEASE_BRANCH unchanged at ${NEW:0:10}"
elif [ "$PUSH" = 1 ]; then
  log "pushing $RELEASE_BRANCH ${OLD:0:10} -> ${NEW:0:10}"
  g push --quiet --force-with-lease="refs/heads/$RELEASE_BRANCH:$OLD" "$FORK_REMOTE" "$NEW:refs/heads/$RELEASE_BRANCH"
else
  log "$RELEASE_BRANCH would move ${OLD:0:10} -> ${NEW:0:10} (not pushed: no --push)"
fi
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  { echo "sha=$NEW"; echo "changed=$CHANGED"; } >>"$GITHUB_OUTPUT"
fi
log "manifest: $OUT/manifest.md"
echo "$NEW"
