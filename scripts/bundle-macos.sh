#!/usr/bin/env bash
#
# Build Mole.app and a DMG from it.
#
# Two modes, chosen by what is in the environment rather than by a flag, so the
# same script serves a laptop and a release runner:
#
#   * Unsigned — no APPLE_SIGNING_IDENTITY. Produces a DMG that runs after the
#     usual right-click → Open, which is all a development build needs.
#   * Signed and notarized — APPLE_SIGNING_IDENTITY plus notarytool credentials.
#     Produces a DMG that opens on any Mac with no warning.
#
# Nothing here reads a secret from a file in the repository, and no credential
# is ever passed on a command line where `ps` could see it.
#
# Usage:
#   scripts/bundle-macos.sh [--version <v>] [--channel stable|beta]
#                           [--out <dir>] [--skip-build]
#
# Environment (all optional; absent means "unsigned"):
#   APPLE_SIGNING_IDENTITY  Developer ID Application identity in the keychain.
#   APPLE_API_KEY_PATH      App Store Connect .p8 key, with:
#   APPLE_API_KEY_ID
#   APPLE_API_ISSUER
#   — or the Apple ID route:
#   APPLE_ID, APPLE_APP_PASSWORD, APPLE_TEAM_ID
#   BUNDLE_ID               Defaults to com.trongduong.mole.

set -euo pipefail

# Never trace: a signing identity and an app-specific password would end up in
# the log the moment someone turns this on to debug it.
set +x

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APP_NAME="Mole"
BINARY_NAME="Mole"
BUNDLE_ID="${BUNDLE_ID:-com.trongduong.mole}"

VERSION=""
CHANNEL="stable"
OUT_DIR="$ROOT/dist"
SKIP_BUILD="false"

die() { printf 'error: %s\n' "$1" >&2; exit 1; }
note() { printf '==> %s\n' "$1"; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="${2:-}"; shift 2 ;;
        --channel) CHANNEL="${2:-}"; shift 2 ;;
        --out) OUT_DIR="${2:-}"; shift 2 ;;
        --skip-build) SKIP_BUILD="true"; shift ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

[[ "$(uname -s)" == "Darwin" ]] || die "this script builds a macOS bundle and needs macOS"
[[ "$CHANNEL" == "stable" || "$CHANNEL" == "beta" ]] || die "channel must be stable or beta"

# The version the binary reports is the one compiled into it, so the bundle
# takes its version from the same place rather than being told twice.
if [[ -z "$VERSION" ]]; then
    VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)"
fi
[[ -n "$VERSION" ]] || die "could not determine the version"

# Apple's version keys must be dotted integers; the channel suffix lives in our
# own keys instead. See packaging/macos/Info.plist.in.
SHORT_VERSION="${VERSION%%-*}"
ARCH="$(uname -m)"

APP_DIR="$OUT_DIR/$APP_NAME.app"
DMG_PATH="$OUT_DIR/$APP_NAME-$VERSION-$ARCH.dmg"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

if [[ "$SKIP_BUILD" != "true" ]]; then
    note "Building $BINARY_NAME $VERSION ($CHANNEL) for $ARCH"
    cargo build --release --locked
fi

BINARY="$ROOT/target/release/$BINARY_NAME"
[[ -f "$BINARY" ]] || die "no release binary at $BINARY"

# ---------------------------------------------------------------------------
# Bundle
# ---------------------------------------------------------------------------

note "Assembling $APP_NAME.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp "$BINARY" "$APP_DIR/Contents/MacOS/$BINARY_NAME"
chmod +x "$APP_DIR/Contents/MacOS/$BINARY_NAME"

sed -e "s|@BUNDLE_ID@|$BUNDLE_ID|g" \
    -e "s|@SHORT_VERSION@|$SHORT_VERSION|g" \
    -e "s|@VERSION@|$VERSION|g" \
    -e "s|@CHANNEL@|$CHANNEL|g" \
    packaging/macos/Info.plist.in > "$APP_DIR/Contents/Info.plist"

printf 'APPL????' > "$APP_DIR/Contents/PkgInfo"

# The icon is optional: a bundle without one gets the generic app icon, which
# is a better outcome than refusing to produce a build.
if [[ -f "assets/AppIcon.icns" ]]; then
    cp "assets/AppIcon.icns" "$APP_DIR/Contents/Resources/AppIcon.icns"
else
    note "No assets/AppIcon.icns — bundling without an icon"
fi

# ---------------------------------------------------------------------------
# Signing and notarization
#
# `notarize` takes a path and blocks until Apple has an answer, which is the
# only way to know a build is distributable before publishing it.
# ---------------------------------------------------------------------------

SIGNED="false"

sign() {
    local path="$1"
    codesign --force --timestamp --options runtime \
        --sign "$APPLE_SIGNING_IDENTITY" "$path"
}

notary_args() {
    # Prefer the App Store Connect key: it is scoped, revocable, and needs no
    # Apple ID password anywhere near the runner.
    if [[ -n "${APPLE_API_KEY_PATH:-}" ]]; then
        printf '%s\0' --key "$APPLE_API_KEY_PATH" \
            --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER"
    elif [[ -n "${APPLE_ID:-}" ]]; then
        printf '%s\0' --apple-id "$APPLE_ID" \
            --password "$APPLE_APP_PASSWORD" --team-id "$APPLE_TEAM_ID"
    fi
}

can_notarize() {
    [[ -n "${APPLE_API_KEY_PATH:-}" && -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER:-}" ]] ||
        [[ -n "${APPLE_ID:-}" && -n "${APPLE_APP_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]
}

notarize() {
    local path="$1"
    local args=()
    while IFS= read -r -d '' arg; do args+=("$arg"); done < <(notary_args)

    note "Notarizing $(basename "$path") — this waits on Apple"
    xcrun notarytool submit "$path" "${args[@]}" --wait --timeout 30m
    xcrun stapler staple "$path"
}

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
    note "Signing $APP_NAME.app"
    sign "$APP_DIR"
    codesign --verify --strict --verbose=2 "$APP_DIR"
    SIGNED="true"

    if can_notarize; then
        # notarytool takes an archive, not a bundle, so the app goes to Apple
        # zipped and the staple lands on the bundle itself.
        ZIP_PATH="$OUT_DIR/$APP_NAME-$VERSION-$ARCH.zip"
        /usr/bin/ditto -c -k --keepParent "$APP_DIR" "$ZIP_PATH"
        notarize "$ZIP_PATH" || die "notarization of the app failed"
        # The staple has to go on the bundle; the zip was only the envelope.
        xcrun stapler staple "$APP_DIR"
        rm -f "$ZIP_PATH"
    else
        note "No notarization credentials — signed but not notarized"
    fi
else
    note "No APPLE_SIGNING_IDENTITY — building unsigned (development build)"
fi

# ---------------------------------------------------------------------------
# DMG
# ---------------------------------------------------------------------------

note "Creating $(basename "$DMG_PATH")"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

cp -R "$APP_DIR" "$STAGING/"
# The conventional drag-to-install target.
ln -s /Applications "$STAGING/Applications"

rm -f "$DMG_PATH"
hdiutil create \
    -volname "$APP_NAME $VERSION" \
    -srcfolder "$STAGING" \
    -fs HFS+ \
    -format UDZO \
    -ov \
    "$DMG_PATH" >/dev/null

if [[ "$SIGNED" == "true" ]]; then
    note "Signing the DMG"
    sign "$DMG_PATH"
    if can_notarize; then
        notarize "$DMG_PATH" || die "notarization of the DMG failed"
    fi
fi

note "Built $DMG_PATH"

# Consumed by the release workflow, which attaches these to the GitHub release.
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    {
        echo "dmg_path=$DMG_PATH"
        echo "dmg_name=$(basename "$DMG_PATH")"
        echo "app_path=$APP_DIR"
        echo "signed=$SIGNED"
    } >> "$GITHUB_OUTPUT"
fi
