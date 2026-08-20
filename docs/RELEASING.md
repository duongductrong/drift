# Releasing

Releases are cut by hand, one channel at a time, from the **Release** workflow
in the Actions tab. Nothing publishes on a push or a tag: `workflow_dispatch`
is the only trigger, and GitHub restricts it to accounts with write access.

## Cutting a release

1. Actions → **Release** → *Run workflow*.
2. Fill in the three inputs:

   | Input | Meaning |
   | --- | --- |
   | `channel` | `beta` publishes a GitHub **pre-release**; `stable` publishes a normal release. |
   | `version` | Optional. Empty means "use `Cargo.toml`'s version"; on the beta channel the next free `-beta.N` suffix is appended. |
   | `draft` | Publish as a draft so the generated notes can be edited, then release by hand. |

3. The run tests, builds, signs (when configured), notarizes, verifies the DMG,
   generates the notes, and creates the release with the DMG attached.

The generated notes are also printed to the run summary, so you can read them
without opening the release.

### What the version input accepts

`0.2.0`, `v0.2.0`, and `0.2.0-beta.1` are all fine. Two rules are enforced
before anything is built, because both would produce a release the update
checker mishandles:

* a **stable** release may not carry a pre-release suffix;
* a **beta** release must — one is added if you leave it off.

An existing tag stops the run rather than being overwritten.

## The metadata contract

`src/core/update.rs` reads GitHub's releases API and decides what to offer.
What it relies on:

| Field | Beta | Stable |
| --- | --- | --- |
| Tag | `v<semver>` with a pre-release suffix, e.g. `v0.2.0-beta.1` | `v<semver>`, e.g. `v0.2.0` |
| GitHub `prerelease` flag | true | false |
| DMG asset | `Drift-<version>-<arch>.dmg` | same |
| `Info.plist` `DriftReleaseChannel` | `beta` | `stable` |
| `Info.plist` `DriftReleaseVersion` | full version, suffix included | full version |

The channel is carried twice on purpose — in the tag and in the pre-release
flag — and either one is enough to keep a build away from stable users. The
workflow verifies the mounted DMG's plist against the release it is about to
publish, so an artifact and its release cannot disagree.

Drafts are invisible to the API the app reads, so a draft release is never
offered to anyone until you publish it.

## Release notes

`.github/scripts/release-notes.sh` groups the commit subjects since the
previous release into **Features**, **Improvements**, **Fixes** and **Other
Changes**, reading conventional-commit prefixes. Build, CI, test, chore and
version-bump commits are dropped; only subjects are used, never commit bodies.

Where the range starts depends on the channel: a **stable** release measures
from the last stable release — so work that shipped through the betas in
between is described again for people who never ran one — and a **beta**
measures from the last release of any kind.

Preview what a release would say, before running anything:

```bash
.github/scripts/release-notes.sh --version 0.2.0 --channel stable
```

## Signing and notarization

Everything below is optional. With none of it configured the workflow still
produces a working DMG — just an unsigned one, which macOS asks about on first
open. Configure them on the `release` environment (or as repository secrets).

| Secret | What it is |
| --- | --- |
| `APPLE_CERTIFICATE_P12` | Developer ID Application certificate and key, exported as `.p12`, base64-encoded. |
| `APPLE_CERTIFICATE_PASSWORD` | The password set when exporting it. |
| `APPLE_SIGNING_IDENTITY` | The identity name, e.g. `Developer ID Application: Your Name (TEAM123456)`. |
| `APPLE_API_KEY` | App Store Connect API key (`.p8`), base64-encoded. |
| `APPLE_API_KEY_ID` | That key's ID. |
| `APPLE_API_ISSUER` | The issuer UUID it belongs to. |

Optional variable: `BUNDLE_ID` (defaults to `com.trongduong.drift`).

```bash
# Producing the two base64 blobs
base64 -i certificate.p12 | pbcopy
base64 -i AuthKey_XXXXXXXX.p8 | pbcopy
```

The certificate is imported into a keychain created for the job and deleted
afterwards, the notarization key is written to `RUNNER_TEMP` and removed in the
same step, and nothing is ever passed where a log or `ps` could capture it. The
scripts refuse to run under `set -x` for the same reason.

If only the certificate is configured, the build is signed but not notarized —
useful while waiting on an Apple Developer account.

## Building locally

```bash
scripts/bundle-macos.sh                      # unsigned, version from Cargo.toml
scripts/bundle-macos.sh --channel beta \
    --version 0.2.0-beta.1                   # what a beta run produces
```

The result lands in `dist/`. An unsigned build opens after a right-click →
**Open**, or:

```bash
xattr -dr com.apple.quarantine /Applications/Drift.app
```

CI can build the same thing: Actions → **CI** → *Run workflow* uploads an
unsigned DMG as an artifact.

## Checking the checker

```bash
cargo test                                   # the channel and version rules
cargo test -- --ignored --nocapture          # a real request to this repo
DRIFT_UPDATE_REPO=zed-industries/zed \
    cargo test -- --ignored --nocapture      # against a repo publishing both kinds
```

`DRIFT_UPDATE_REPO` is read at build time, so a fork's builds check the fork's
releases; the release workflow sets it to `github.repository` automatically.
