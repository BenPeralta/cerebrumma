#!/bin/sh
# Cerebrumma Installer
# Usage: curl -fsSL https://get.cerebrumma.com | sh
set -e

REPO="https://github.com/BenPeralta/cerebrumma"
CLAUDE_JSON="$HOME/.claude.json"

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

# ── Platform check ────────────────────────────────────────────────────────────
OS=$(uname -s)
case "$OS" in
  Darwin) ;;
  Linux)  ;;
  *)      die "Unsupported OS: $OS" ;;
esac

# ── 1. Rust ───────────────────────────────────────────────────────────────────
if command -v cargo >/dev/null 2>&1; then
  ok "Rust already installed"
else
  info "Installing Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
  ok "Rust installed"
fi

# ── 2. uv ─────────────────────────────────────────────────────────────────────
if command -v uv >/dev/null 2>&1; then
  ok "uv already installed"
else
  info "Installing uv (Python package manager)..."
  curl -LsSf https://astral.sh/uv/install.sh | sh
  export PATH="$HOME/.local/bin:$PATH"
  ok "uv installed"
fi

# ── 3. Clone repo ─────────────────────────────────────────────────────────────
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

info "Cloning Cerebrumma..."
git clone --depth 1 --quiet "$REPO" "$TMPDIR/cerebrumma" || die "git clone failed — is git installed?"
ok "Cloned $REPO"

# ── 4. Build + install CLI ────────────────────────────────────────────────────
info "Building cerebrum CLI (this takes ~30s on first run)..."
cargo install --path "$TMPDIR/cerebrumma" --quiet
CEREBRUM_BIN=$(command -v cerebrum)
ok "cerebrum installed → $CEREBRUM_BIN"

# ── 5. Install MCP server ─────────────────────────────────────────────────────
MCP_SRC="$TMPDIR/cerebrumma/mcp_server"
MCP_DEST="$HOME/.cerebrumma/mcp_server"

info "Installing MCP server..."
mkdir -p "$HOME/.cerebrumma"
cp -r "$MCP_SRC" "$MCP_DEST"
uv pip install --quiet -e "$MCP_DEST" 2>/dev/null \
  || uv pip install --system --quiet -e "$MCP_DEST" 2>/dev/null \
  || warn "MCP server install skipped — run: uv pip install -e ~/.cerebrumma/mcp_server"
ok "MCP server installed → $MCP_DEST"

# ── 6. Configure Claude Code (optional) ──────────────────────────────────────
configure_claude() {
  # Check if claude CLI is available
  if ! command -v claude >/dev/null 2>&1; then
    warn "Claude Code CLI not found — skipping MCP auto-config"
    warn "After installing Claude Code, run: claude mcp add cerebrumma -s user $MCP_DEST/run-mcp.sh"
    return
  fi

  # Check if already configured
  if grep -q "cerebrumma" "$CLAUDE_JSON" 2>/dev/null; then
    ok "Claude Code already configured"
    return
  fi

  # Write wrapper script (avoids --project flag clash with claude mcp add parser)
  WRAPPER="$HOME/.cerebrumma/run-mcp.sh"
  UV_BIN=$(command -v uv || echo "$HOME/.local/bin/uv")
  printf '#!/bin/sh\n%s run --project %s cerebrumma-mcp\n' "$UV_BIN" "$MCP_DEST" > "$WRAPPER"
  chmod +x "$WRAPPER"

  claude mcp add cerebrumma -s user "$WRAPPER"
  ok "Claude Code configured — restart Claude Code to load the Brain"
}

info "Configuring Claude Code MCP integration..."
configure_claude

# ── 7. Initialize global Brain ────────────────────────────────────────────────
if [ ! -d "$HOME/.cerebrum" ]; then
  info "Initializing global Brain..."
  cerebrum init --global
  ok "Global Brain created at ~/.cerebrum/"
else
  ok "Global Brain already exists at ~/.cerebrum/"
fi

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
printf "    ${BOLD}cerebrum dream${NC}           ${DIM}# run a reflection cycle${NC}\n"
printf "\n"
printf "  ${DIM}Restart Claude Code to activate the MCP Brain tools.${NC}\n"
printf "  ${DIM}Docs: $REPO${NC}\n\n"
