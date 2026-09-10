#!/usr/bin/env bash
set -euo pipefail

# Record which commits of the two sibling checkouts the Play workflow builds
# against, or check that the record still matches them.
#
# `Cargo.toml` names rinch and rhypedb by path — `../rinch-fixes` and
# `../rhypedb-main` — which is right for a laptop, where a framework fix is an
# edit in the next directory over. It says nothing about *which* commit,
# though, and a GitHub runner has no next directory. `.github/workflows/
# play.yml` clones both beside this repo at the revisions in
# `.github/siblings.env`, and this script is what writes them.
#
# The failure it exists for is the quiet one. Build and try a bundle here
# against a rinch commit the pin does not name, tag the release, and the runner
# builds the same app source against the *pinned* rinch — which will very
# likely compile, and ship without whatever fix the laptop build had in it.
#
#   scripts/pin-siblings.sh           # pin both to their current HEADs
#   scripts/pin-siblings.sh --check   # exit 1 if either HEAD has left its pin
#
# Pinning refuses a commit the runner could not fetch: it has to be reachable
# from some branch of the repository siblings.env names, which for rinch means
# `git -C ../rinch-fixes push fork local/both-fixes` first.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
PINS="$ROOT/.github/siblings.env"

CHECK=false
if [[ "${1:-}" == "--check" ]]; then
    CHECK=true
fi

pinned() { sed -n "s/^$1=//p" "$PINS"; }

STATUS=0
for sibling in "RINCH rinch-fixes" "RHYPEDB rhypedb-main"; do
    read -r key dir <<<"$sibling"
    checkout="$ROOT/../$dir"
    repo="$(pinned "${key}_REPO")"
    head="$(git -C "$checkout" rev-parse HEAD)"

    if [[ "$CHECK" == true ]]; then
        if [[ "$head" != "$(pinned "${key}_REV")" ]]; then
            echo "$dir: HEAD is $head, but the Play workflow builds $(pinned "${key}_REV")"
            STATUS=1
        fi
        continue
    fi

    if [[ -n "$(git -C "$checkout" status --porcelain --untracked-files=no)" ]]; then
        echo "WARNING: $dir has uncommitted changes; the pin is its HEAD, not its working tree"
    fi

    # Every branch of the remote, into a namespace of its own and gone again
    # afterwards, so the question "could a runner fetch this?" is answered by
    # the remote and not by whatever this checkout happens to have fetched.
    git -C "$checkout" fetch -q "https://github.com/$repo.git" "+refs/heads/*:refs/pin-check/*"
    reachable="$(git -C "$checkout" for-each-ref --contains "$head" refs/pin-check)"
    git -C "$checkout" for-each-ref --format='%(refname)' refs/pin-check \
        | xargs -r -n1 git -C "$checkout" update-ref -d
    if [[ -z "$reachable" ]]; then
        echo "ERROR: $dir's HEAD $head is on no branch of github.com/$repo; push it first"
        exit 1
    fi

    sed -i "s/^${key}_REV=.*/${key}_REV=$head/" "$PINS"
    echo "$dir: pinned $head"
done
exit "$STATUS"
