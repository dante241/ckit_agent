#!/bin/sh
#
# ckit standalone installer — PUBLIC GitHub repo (dante241/ckit_agent).
#
# Downloads the prebuilt `ckit` binary from the latest GitHub Release — no git
# clone, no Rust toolchain, no cargo build, no token. Ideal for a fresh machine.
#
#   curl -fsSL https://raw.githubusercontent.com/dante241/ckit_agent/main/install.sh | sh
#
# Upgrade:   re-run the same command (atomically replaces the old binary).
# Uninstall: curl -fsSL .../install.sh | sh -s -- --uninstall
#
# Environment:
#   CKIT_GITHUB_TOKEN  optional — only to dodge GitHub's 60-req/hour anonymous
#                      API limit on shared/CI hosts. GITHUB_TOKEN also accepted.
#   CKIT_VERSION       release tag to install (default: latest, e.g. v0.53.0)
#   CKIT_BIN_DIR       install location (default: ~/.local/bin)
set -eu

REPO="dante241/ckit_agent"
API="https://api.github.com/repos/$REPO"
BIN_DIR="${CKIT_BIN_DIR:-$HOME/.local/bin}"
BIN="$BIN_DIR/ckit"

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$BIN"
  echo "ckit uninstalled (removed $BIN)."
  exit 0
fi

# Optional token — public repo needs none; only lifts the anon API rate limit.
TOKEN="${CKIT_GITHUB_TOKEN:-${GITHUB_TOKEN:-}}"
auth_header() { [ -n "$TOKEN" ] && printf 'Authorization: Bearer %s' "$TOKEN" || printf ''; }

# 1. Platform check — resolve os first, then arch (arm64 naming differs per-os).
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux) os="linux" ;;
  Darwin) os="darwin" ;;
  *) echo "ckit: no prebuilt binary for '$os' yet — build from source: https://github.com/$REPO (scripts/bootstrap.sh)" >&2; exit 1 ;;
esac
case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  aarch64|arm64)
    case "$os" in
      linux) arch="aarch64" ;;
      darwin) arch="arm64" ;;
    esac
    ;;
  *) echo "ckit: no prebuilt binary for '$arch' yet — build from source: https://github.com/$REPO (scripts/bootstrap.sh)" >&2; exit 1 ;;
esac

# 2. Resolve the release JSON (latest unless CKIT_VERSION is pinned).
version="${CKIT_VERSION:-}"
if [ -n "$version" ]; then
  case "$version" in v*) ;; *) version="v$version" ;; esac
  rel_url="$API/releases/tags/$version"
else
  rel_url="$API/releases/latest"
fi
h="$(auth_header)"
if [ -n "$h" ]; then
  release_json="$(curl -fsSL -H "$h" -H "Accept: application/vnd.github+json" -H "User-Agent: ckit-installer" "$rel_url")" \
    || { echo "ckit: could not query release ($rel_url)." >&2; exit 1; }
else
  release_json="$(curl -fsSL -H "Accept: application/vnd.github+json" -H "User-Agent: ckit-installer" "$rel_url")" \
    || { echo "ckit: could not query release ($rel_url)." >&2; exit 1; }
fi

tag="$(printf '%s' "$release_json" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n1)"
[ -n "$tag" ] || { echo "ckit: could not resolve release tag." >&2; exit 1; }
asset="ckit-${tag}-${os}-${arch}"

# 3. Public repo → the browser download URL needs no auth. Pull it straight out
#    of the release JSON for the object whose name matches our platform.
url="$(printf '%s' "$release_json" \
  | tr ',' '\n' \
  | grep 'browser_download_url' \
  | sed -n 's/.*"browser_download_url": *"\([^"]*'"$asset"'\)".*/\1/p' \
  | head -n1)"
[ -n "$url" ] || { echo "ckit: release $tag has no asset '$asset'." >&2; exit 1; }

# 4. Download the asset, then atomically replace.
echo "Installing ckit $tag ($os-$arch)..."
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
curl -fSL -H "User-Agent: ckit-installer" "$url" -o "$tmp" 2>/dev/null \
  || { echo "ckit: download failed: $url" >&2; exit 1; }
[ -s "$tmp" ] || { echo "ckit: downloaded an empty file from $url" >&2; exit 1; }
chmod 0755 "$tmp"
mkdir -p "$BIN_DIR"
mv -f "$tmp" "$BIN"
trap - EXIT

echo "Installed → $BIN"
"$BIN" --version 2>/dev/null || true

# 5. PATH hint if ~/.local/bin is not yet on PATH (bash/zsh/fish).
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    echo ""
    echo "$BIN_DIR is not on your PATH. Add it:"
    echo "  bash/zsh: echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> ~/.bashrc   # or ~/.zshrc"
    echo "  fish:     fish_add_path -aP $BIN_DIR"
    echo "  (\`ckit setup\` also wires PATH for bash/zsh/fish automatically.)"
    ;;
esac
echo ""
echo "Done. Next steps:"
echo "  ckit setup        # full stack + config"
echo "  ckit doctor       # verify"
echo "  ckit up           # upgrade later (or re-run this installer)"
