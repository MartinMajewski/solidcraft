#!/usr/bin/env bash
# Build a personal macOS SolidCraft from a checkout (normally of mm/releases), branded so it can sit
# next to an official install:
#
#   $OUT/solidcraft-mm-<version>-macos-<arch>.dmg      "SolidCraft MM.app" on a drag-to-Applications DMG
#   $OUT/solidcraft-mm-cli-<version>-macos-<arch>.zip  the headless CLI
#   $OUT/version, $OUT/tag                             the version string and the release tag
#
# <version> is <upstream version>+mm.<UTC date>.<commit>, e.g. 0.40.0-rc.2+mm.20261105.ec4d018;
# the tag is mm-<upstream version>-<date>-<commit>, e.g. mm-0.40.0-rc.2-20261105-ec4d018.
#
# It runs upstream's packaging/macos/package.sh unchanged (SOLIDCRAFT_VERSION carries the version
# into the binary), then copies the app it assembled, renames it, gives it its own bundle id,
# re-signs it ad-hoc and puts it on a plain DMG. Upstream files are never edited.
#
# Usage: mm/build-macos.sh [--src DIR] [--out DIR] [--arch aarch64|x86_64|universal] [--install]
#   --src DIR    the SolidCraft checkout to build (default: the current directory)
#   --out DIR    output directory (default: <src>/dist/mm)
#   --arch A     default: this Mac's architecture
#   --install    also copy the app to /Applications (replaces only an earlier "SolidCraft MM.app")
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=config.sh
. "$HERE/config.sh"

SRC=.
OUT=
INSTALL=0
case "$(uname -m)" in arm64) ARCH=aarch64 ;; *) ARCH=x86_64 ;; esac
while [ $# -gt 0 ]; do
  case "$1" in
    --src) SRC="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --arch) ARCH="$2"; shift 2 ;;
    --install) INSTALL=1; shift ;;
    -h | --help) sed -n '2,21p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = Darwin ] || { echo "macOS only" >&2; exit 2; }
SRC="$(cd "$SRC" && git rev-parse --show-toplevel)"
[ -x "$SRC/packaging/macos/package.sh" ] || { echo "$SRC is not a SolidCraft checkout" >&2; exit 2; }
OUT="${OUT:-$SRC/dist/mm}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
log() { printf '==> %s\n' "$*" >&2; }

# ---- version ------------------------------------------------------------------------------------
BASE="$(awk '/^\[/ { in_pkg = ($0 == "[workspace.package]"); next }
  in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' "$SRC/Cargo.toml")"
[ -n "$BASE" ] || { echo "no [workspace.package] version in $SRC/Cargo.toml" >&2; exit 2; }
DATE="$(date -u +%Y%m%d)"
SHA7="$(git -C "$SRC" rev-parse --short=7 HEAD)"
VERSION="$BASE+$BUILD_LABEL.$DATE.$SHA7"
TAG="$BUILD_LABEL-$BASE-$DATE-$SHA7"
SHORT="${BASE%%-*}"
SHORT="${SHORT%%+*}"
log "building $APP_NAME $VERSION ($ARCH) from $SRC"

# ---- upstream packaging -------------------------------------------------------------------------
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$SRC/target}"
PKG_OUT="$OUT/upstream-package"
rm -rf "$PKG_OUT"
(cd "$SRC" && SOLIDCRAFT_VERSION="$VERSION" DIST="$PKG_OUT" ./packaging/macos/package.sh --arch "$ARCH")

# ---- rebrand ------------------------------------------------------------------------------------
STAGE="$OUT/stage"
APP="$STAGE/$APP_NAME.app"
rm -rf "$STAGE"
mkdir -p "$STAGE"
ditto "$CARGO_TARGET_DIR/macos-package/SolidCraft.app" "$APP"
PLIST="$APP/Contents/Info.plist"
pb() { /usr/libexec/PlistBuddy -c "$1" "$PLIST"; }
pb "Set :CFBundleName $APP_NAME"
pb "Set :CFBundleDisplayName $APP_NAME"
pb "Set :CFBundleIdentifier $BUNDLE_ID"
pb "Set :CFBundleShortVersionString $SHORT"
pb "Set :CFBundleVersion $SHORT.$DATE"
plutil -lint "$PLIST"
# Ad-hoc signatures (Apple silicon refuses unsigned code); nested code first.
codesign --force --sign - --timestamp=none --identifier "$BUNDLE_ID.cli" "$APP/Contents/MacOS/solidcraft-cli"
codesign --force --sign - --timestamp=none "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"
"$APP/Contents/MacOS/SolidCraft" --version
# `--version` prints the crate version; the build string is what About and the update check use.
[ "$(pb 'Print :SolidCraftVersion')" = "$VERSION" ] || { echo "Info.plist does not carry $VERSION" >&2; exit 1; }
LC_ALL=C grep -qaF "$VERSION" "$APP/Contents/MacOS/SolidCraft" || { echo "the binary was not built with $VERSION" >&2; exit 1; }

# ---- DMG and CLI --------------------------------------------------------------------------------
DMG="$OUT/solidcraft-$BUILD_LABEL-$VERSION-macos-$ARCH.dmg"
CLI_ZIP="$OUT/solidcraft-$BUILD_LABEL-cli-$VERSION-macos-$ARCH.zip"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -quiet -volname "$APP_NAME" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG"
mv "$PKG_OUT/solidcraft-cli-$VERSION-macos-$ARCH.zip" "$CLI_ZIP"
rm -rf "$PKG_OUT"
echo "$VERSION" >"$OUT/version"
echo "$TAG" >"$OUT/tag"
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  { echo "version=$VERSION"; echo "tag=$TAG"; echo "dmg=$DMG"; echo "cli_zip=$CLI_ZIP"; } >>"$GITHUB_OUTPUT"
fi

# ---- install ------------------------------------------------------------------------------------
if [ "$INSTALL" = 1 ]; then
  DEST="/Applications/$APP_NAME.app"
  if [ -e "$DEST" ]; then
    have="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$DEST/Contents/Info.plist" 2>/dev/null || true)"
    [ "$have" = "$BUNDLE_ID" ] || { echo "$DEST exists and is not an earlier $APP_NAME build; not replacing it" >&2; exit 1; }
    rm -rf "$DEST"
  fi
  ditto "$APP" "$DEST"
  log "installed $DEST"
fi
log "done: $DMG"
