#!/usr/bin/env bash
#
# Generate the release notes for one release, on stdout.
#
# Why not GitHub's own generator: it groups by pull request label, and this
# repository lands most work as commits straight on `main`. Given that, GitHub
# produces a flat "what's changed" list of raw commit subjects — the dump this
# is meant to avoid. Conventional-commit prefixes are already the convention
# here, so they are what the grouping reads.
#
# What ends up in the notes is deliberately narrow: commit *subjects* only,
# never bodies. Bodies are where branch names, internal reasoning, and pasted
# output live, and none of that belongs in front of a user.
#
# Usage:
#   .github/scripts/release-notes.sh --version 0.2.0 --channel beta \
#       [--previous <tag>] [--repo owner/name]
#
# Run it locally to preview what a release would say:
#   .github/scripts/release-notes.sh --version 0.2.0 --channel beta

set -euo pipefail

VERSION=""
CHANNEL="stable"
PREVIOUS=""
REPO="${GITHUB_REPOSITORY:-}"

die() { printf 'error: %s\n' "$1" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="${2:-}"; shift 2 ;;
        --channel) CHANNEL="${2:-}"; shift 2 ;;
        --previous) PREVIOUS="${2:-}"; shift 2 ;;
        --repo) REPO="${2:-}"; shift 2 ;;
        -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

[[ -n "$VERSION" ]] || die "--version is required"
[[ "$CHANNEL" == "stable" || "$CHANNEL" == "beta" ]] || die "channel must be stable or beta"

TAG="v$VERSION"

# ---------------------------------------------------------------------------
# Where the range starts
#
# A stable release measures from the last *stable* one, so everything that went
# out through the betas in between is described again for the people who never
# ran a beta. A beta measures from the last release of any kind, so its notes
# only cover what is actually new since the previous beta.
# ---------------------------------------------------------------------------

find_previous_tag() {
    local tag
    # Ordered by when the tag was made rather than by version string: git's
    # version sort places `v1.0.0-beta.1` *after* `v1.0.0`, which would pick
    # the wrong starting point every time a beta line precedes a release.
    while read -r tag; do
        [[ -n "$tag" && "$tag" != "$TAG" ]] || continue
        # A stable release ignores the beta tags between it and the last one.
        if [[ "$CHANNEL" == "stable" && "$tag" == *-* ]]; then
            continue
        fi
        printf '%s\n' "$tag"
        return
    done < <(git tag --list 'v*' --merged HEAD --sort=-creatordate)
}

if [[ -z "$PREVIOUS" ]]; then
    PREVIOUS="$(find_previous_tag || true)"
fi

if [[ -n "$PREVIOUS" ]]; then
    RANGE="$PREVIOUS..HEAD"
else
    # First release: everything is new.
    RANGE="HEAD"
fi

# ---------------------------------------------------------------------------
# Grouping
#
# One pass over the subjects, sorting each into the section a user would look
# for it in. Anything that only affects how the project is built or tested is
# dropped: it is real work, but it is not news to someone downloading an app.
# ---------------------------------------------------------------------------

features=()
improvements=()
fixes=()
other=()

# "feat(ui)!: Add a thing" -> type "feat", subject "Add a thing"
commit_type() {
    local subject="$1"
    if [[ "$subject" =~ ^([a-zA-Z]+)(\([^\)]*\))?!?:[[:space:]] ]]; then
        printf '%s' "${BASH_REMATCH[1]}" | tr '[:upper:]' '[:lower:]'
    fi
}

strip_prefix() {
    local subject="$1"
    printf '%s' "${subject#*: }"
}

# One readable line: sentence case, no trailing period, and no runaway length.
tidy() {
    local text="$1"
    # git's "subject" is the first *paragraph*, so a commit whose body was not
    # separated by a blank line arrives with its bullet list inlined. Keep the
    # headline and drop the list — the detail belongs in the commit, not here.
    text="${text%% - *}"
    text="${text%"${text##*[![:space:]]}"}"
    text="${text%.}"
    if ((${#text} > 120)); then
        text="${text:0:119}…"
    fi
    printf '%s%s' "$(printf '%s' "${text:0:1}" | tr '[:lower:]' '[:upper:]')" "${text:1}"
}

# `--format` (unlike `--pretty=format:`) terminates the last line, and the
# `-n` guard covers a stream that still arrives without one — between them the
# oldest commit in the range cannot be silently dropped.
while IFS= read -r subject || [[ -n "$subject" ]]; do
    [[ -n "$subject" ]] || continue
    # Merge commits describe the merge, not the change.
    [[ "$subject" == Merge\ * ]] && continue

    type="$(commit_type "$subject")"
    case "$type" in
        # Build plumbing, test scaffolding and the release commit itself are
        # invisible to a user by definition.
        chore|ci|build|test|release) continue ;;
    esac

    # A version bump is an artifact of releasing, whatever it is labelled.
    shopt -s nocasematch
    if [[ "$subject" =~ (bump|release)[[:space:]]+(the[[:space:]]+)?version ]]; then
        shopt -u nocasematch
        continue
    fi
    shopt -u nocasematch

    if [[ -n "$type" ]]; then
        entry="$(tidy "$(strip_prefix "$subject")")"
    else
        entry="$(tidy "$subject")"
    fi
    [[ -n "$entry" ]] || continue

    case "$type" in
        feat) features+=("$entry") ;;
        fix) fixes+=("$entry") ;;
        perf|refactor|style|improvement|improve) improvements+=("$entry") ;;
        *) other+=("$entry") ;;
    esac
done < <(git log --no-merges --format='%s' "$RANGE")

section() {
    local title="$1"; shift
    (($# > 0)) || return 0
    printf '### %s\n\n' "$title"
    printf -- '- %s\n' "$@"
    printf '\n'
}

# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------

if [[ "$CHANNEL" == "beta" ]]; then
    printf '## Drift %s — Beta\n\n' "$VERSION"
    printf 'A pre-release build. It gets new work first and may be rough; '
    printf 'people on the Stable channel are not offered it.\n\n'
else
    printf '## Drift %s\n\n' "$VERSION"
fi

if ((${#features[@]} + ${#improvements[@]} + ${#fixes[@]} + ${#other[@]} == 0)); then
    printf 'Maintenance release — no user-facing changes.\n\n'
else
    section "Features" "${features[@]+"${features[@]}"}"
    section "Improvements" "${improvements[@]+"${improvements[@]}"}"
    section "Fixes" "${fixes[@]+"${fixes[@]}"}"
    section "Other Changes" "${other[@]+"${other[@]}"}"
fi

printf '### Install\n\n'
printf 'Download the DMG below, drag Drift to Applications, and open it.\n'
printf 'Drift checks for updates on the **%s** channel by default; ' \
    "$([[ "$CHANNEL" == "beta" ]] && printf 'Beta' || printf 'Stable')"
printf 'you can change that in Settings → Updates.\n\n'

if [[ -n "$PREVIOUS" && -n "$REPO" ]]; then
    printf '**Full changelog**: https://github.com/%s/compare/%s...%s\n' \
        "$REPO" "$PREVIOUS" "$TAG"
elif [[ -n "$PREVIOUS" ]]; then
    printf '**Full changelog**: %s...%s\n' "$PREVIOUS" "$TAG"
fi
