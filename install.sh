#!/bin/sh
# Install hako (箱) from the latest GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/j-kop/hako-source/main/install.sh | sh
#
# Downloads the hako binary for this platform, then lets it install itself through
# the registry (`hako install hako`). That copy is checksum-verified and lives in
# ~/.hako, so later `hako upgrade` runs update hako along with everything else.
set -eu

repo="j-kop/hako-source"

case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *) echo "hako: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) echo "hako: unsupported operating system $(uname -s)" >&2; exit 1 ;;
esac

if ! command -v git >/dev/null 2>&1; then
  echo "hako: git is required; please install it first" >&2
  exit 1
fi

url="https://github.com/$repo/releases/latest/download/hako-$arch-$os.tar.gz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "downloading $url"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$tmp/hako.tar.gz"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$tmp/hako.tar.gz" "$url"
else
  echo "hako: curl or wget is required" >&2
  exit 1
fi

tar xzf "$tmp/hako.tar.gz" -C "$tmp"
"$tmp"/hako-*/hako install hako
