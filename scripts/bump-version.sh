#!/usr/bin/env bash
# Create a version PR for hako: bump the version in Cargo.toml and Cargo.lock on a new branch,
# push it, and print the link for opening the pull request. Merging that PR releases the version.
#
#   scripts/bump-version.sh patch|minor|major|X.Y.Z
#
# Needs: git, cargo, and an up-to-date `main` checked out with no uncommitted changes.
set -euo pipefail

die() { echo "error: $*" >&2; exit 1; }
[ $# -eq 1 ] || { echo "usage: $0 patch|minor|major|X.Y.Z" >&2; exit 2; }

cd "$(git rev-parse --show-toplevel)"
[ -z "$(git status --porcelain)" ] || die "you have uncommitted changes; commit or stash them first"
git fetch --quiet --tags origin main
[ "$(git branch --show-current)" = "main" ] && [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] \
  || die "check out an up-to-date main first: git switch main && git pull"

current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
IFS=. read -r major minor patch <<< "${current%%-*}"
case "$1" in
  major) new="$((major + 1)).0.0" ;;
  minor) new="$major.$((minor + 1)).0" ;;
  patch) new="$major.$minor.$((patch + 1))" ;;
  *)
    printf '%s' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' \
      || die "expected patch, minor, major or a version like 1.2.3, got: $1"
    new="$1"
    ;;
esac

[ "$new" != "$current" ] || die "version is already $current"
[ "$(printf '%s\n%s\n' "$current" "$new" | sort -V | tail -n1)" = "$new" ] \
  || die "$new is lower than the current version $current"
! git rev-parse --verify --quiet "refs/tags/v$new" > /dev/null || die "v$new is already released"

branch="release-v$new"
if git show-ref --verify --quiet "refs/heads/$branch" || git ls-remote --exit-code --heads origin "$branch" > /dev/null; then
  die "branch $branch already exists; is there an open version PR for v$new?"
fi
git switch --quiet -c "$branch"
awk -v v="$new" '!done && /^version = "/ { print "version = \"" v "\""; done = 1; next } { print }' \
  Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
cargo update --workspace --quiet # updates only hako's own entry in Cargo.lock
git commit --quiet -am "Release v$new"
git push --quiet -u origin "$branch"
git switch --quiet main

url=$(git remote get-url origin)
repo=${url#*github.com[:/]}
repo=${repo%.git}
echo "Pushed $branch: $current -> $new"
echo "Open the pull request: https://github.com/$repo/pull/new/$branch"
echo "Merging it releases v$new."
