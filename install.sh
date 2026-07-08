#!/bin/sh
# Cerebrumma Installer
# Usage: curl -fsSL https://get.cerebrumma.com | sh
#
# Downloads a prebuilt `cerebrum` binary for your platform (fast, no compiler).
# Falls back to building from source if no prebuilt binary matches.
set -e

REPO="https://github.com/BenPeralta/cerebrumma"
OWNER="BenPeralta/cerebrumma"
CLAUDE_JSON="$HOME/.claude.json"
BIN_DIR="$HOME/.local/bin"

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

info()    { printf "  ${CYAN}→${NC}  %s\n" "$1"; }
ok()      { printf "  ${GREEN}✓${NC}  %s\n" "$1"; }
warn()    { printf "  ${YELLOW}!${NC}  %s\n" "$1"; }
die()     { printf "  ${RED}✗${NC}  %s\n" "$1" >&2; exit 1; }

# ── Header ────────────────────────────────────────────────────────────────────
printf "\n"
printf "  ${BOLD}${CYAN}Cerebrumma${NC} — Portable, git-tracked AI Brain\n"
printf "  ${DIM}One Brain. Any Tool.${NC}\n\n"

# ── Platform + arch detection ─────────────────────────────────────────────────
OS=$(uname -s)
ARCH=$(uname -m)
TARGET=""
case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
      x86_64)        TARGET="x86_64-apple-darwin" ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64|amd64) TARGET="x86_64-unknown-linux-gnu" ;;
    esac
    ;;
  *)
    die "Unsupported OS: $OS"
    ;;
esac

# Shared temp dir (also used to grab the MCP server source).
SRC_DIR=$(mktemp -d)
trap 'rm -rf "$SRC_DIR"' EXIT

# ── Fallback: build from source (used only if no prebuilt binary applies) ──────
build_from_source() {
  warn "No prebuilt binary for ${OS}/${ARCH} — building from source."

  if command -v cargo >/dev/null 2>&1; then
    ok "Rust already installed"
  else
    info "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
    ok "Rust installed"
  fi

  info "Cloning + building Cerebrumma (this can take a few minutes)..."
  git clone --depth 1 --quiet "$REPO" "$SRC_DIR/repo" || die "git clone failed — is git installed?"
  cargo install --path "$SRC_DIR/repo" --quiet
  ok "cerebrum built + installed"
}

# ── 1. Install the CLI ────────────────────────────────────────────────────────
install_prebuilt() {
  URL="$REPO/releases/latest/download/cerebrum-${TARGET}.tar.gz"
  TARBALL="$SRC_DIR/cerebrum.tar.gz"

  info "Downloading cerebrum (${TARGET})..."
  if ! curl -fL --progress-bar -o "$TARBALL" "$URL"; then
    return 1
  fi

  tar -xzf "$TARBALL" -C "$SRC_DIR" || return 1
  EXTRACTED="$SRC_DIR/cerebrum-${TARGET}/cerebrum"
  [ -f "$EXTRACTED" ] || return 1

  mkdir -p "$BIN_DIR"
  install -m 755 "$EXTRACTED" "$BIN_DIR/cerebrum" 2>/dev/null \
    || { cp "$EXTRACTED" "$BIN_DIR/cerebrum" && chmod 755 "$BIN_DIR/cerebrum"; }
  export PATH="$BIN_DIR:$PATH"
  ok "cerebrum installed → $BIN_DIR/cerebrum"
  return 0
}

if [ -n "$TARGET" ] && install_prebuilt; then
  :
else
  build_from_source
fi

# Make sure we can find it for the rest of this script.
command -v cerebrum >/dev/null 2>&1 || export PATH="$BIN_DIR:$HOME/.cargo/bin:$PATH"
command -v cerebrum >/dev/null 2>&1 || die "cerebrum not found on PATH after install"

# ── 2. uv (for the Python MCP server) ─────────────────────────────────────────
if command -v uv >/dev/null 2>&1; then
  ok "uv already installed"
else
  info "Installing uv (Python package manager)..."
  curl -LsSf https://astral.sh/uv/install.sh | sh
  export PATH="$HOME/.local/bin:$PATH"
  ok "uv installed"
fi

# ── 3. Install MCP server ─────────────────────────────────────────────────────
# Only the small Python folder is needed — no Rust compile here.
MCP_DEST="$HOME/.cerebrumma/mcp_server"
if [ -d "$SRC_DIR/repo/mcp_server" ]; then
  MCP_SRC="$SRC_DIR/repo/mcp_server"           # left over from a source build
else
  info "Fetching MCP server..."
  git clone --depth 1 --quiet "$REPO" "$SRC_DIR/repo" 2>/dev/null && MCP_SRC="$SRC_DIR/repo/mcp_server"
fi

if [ -n "$MCP_SRC" ] && [ -d "$MCP_SRC" ]; then
  info "Installing MCP server..."
  mkdir -p "$HOME/.cerebrumma"
  rm -rf "$MCP_DEST"
  cp -r "$MCP_SRC" "$MCP_DEST"
  uv pip install --quiet -e "$MCP_DEST" 2>/dev/null \
    || uv pip install --system --quiet -e "$MCP_DEST" 2>/dev/null \
    || warn "MCP server install skipped — run: uv pip install -e ~/.cerebrumma/mcp_server"
  ok "MCP server installed → $MCP_DEST"
else
  warn "MCP server source not found — skipping (CLI still works)"
fi

# ── 4. Configure Claude Code (optional) ──────────────────────────────────────
configure_claude() {
  if ! command -v claude >/dev/null 2>&1; then
    warn "Claude Code CLI not found — skipping MCP auto-config"
    warn "After installing Claude Code, run: claude mcp add cerebrumma -s user $MCP_DEST/run-mcp.sh"
    return
  fi

  if grep -q "cerebrumma" "$CLAUDE_JSON" 2>/dev/null; then
    ok "Claude Code already configured"
    return
  fi

  WRAPPER="$HOME/.cerebrumma/run-mcp.sh"
  UV_BIN=$(command -v uv || echo "$HOME/.local/bin/uv")
  printf '#!/bin/sh\n%s run --project %s cerebrumma-mcp\n' "$UV_BIN" "$MCP_DEST" > "$WRAPPER"
  chmod +x "$WRAPPER"

  claude mcp add cerebrumma -s user "$WRAPPER"
  ok "Claude Code configured — restart Claude Code to load the Brain"
}

if [ -d "$MCP_DEST" ]; then
  info "Configuring Claude Code MCP integration..."
  configure_claude
fi

# ── 5. Initialize global Brain ────────────────────────────────────────────────
if [ ! -d "$HOME/.cerebrum" ]; then
  info "Initializing global Brain..."
  cerebrum init --global
  ok "Global Brain created at ~/.cerebrum/"
else
  ok "Global Brain already exists at ~/.cerebrum/"
fi

# ── PATH hint ─────────────────────────────────────────────────────────────────
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    warn "$BIN_DIR is not on your PATH."
    warn "Add this to your shell profile (~/.zshrc or ~/.bashrc):"
    printf "        ${BOLD}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}\n"
    ;;
esac

# ── Done ──────────────────────────────────────────────────────────────────────
printf "\n"
printf "  ${BOLD}${GREEN}Cerebrumma installed!${NC}\n\n"
printf "  Quickstart:\n"
printf "    ${BOLD}cerebrum add --global \"your coding rules\"${NC}\n"
printf "    ${BOLD}cerebrum status${NC}\n"
printf "\n"
printf "  In any project:\n"
printf "    ${BOLD}cerebrum init${NC}            ${DIM}# local Brain for this repo${NC}\n"
printf "    ${BOLD}cerebrum hook install${NC}    ${DIM}# auto-capture git commits${NC}\n"
printf "    ${BOLD}cerebrum map --graph${NC}     ${DIM}# see your Brain as a neural map${NC}\n"
printf "\n"
printf "  ${DIM}Restart Claude Code to activate the MCP Brain tools.${NC}\n"
printf "  ${DIM}Docs: $REPO${NC}\n\n"
