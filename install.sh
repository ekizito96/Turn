#!/bin/sh
# Turn Language Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/ekizito96/Turn/main/install.sh | bash
#
# Installs:
#   turn                     — the core VM and compiler
#   turn-provider-openai     — the default inference provider driver
#
# The infer primitive requires a provider driver in PATH.
# Set TURN_INFER_PROVIDER to switch providers (see https://github.com/ekizito96/Turn/blob/main/PROVIDERS.md)

set -e

REPO="ekizito96/Turn"
INSTALL_DIR="$HOME/.turn/bin"

# ─── Detect platform ──────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux*)   PLATFORM="linux" ;;
  Darwin*)  PLATFORM="macos" ;;
  *)        echo "Error: Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64)  ARCH_NAME="amd64" ;;
  arm64|aarch64) ARCH_NAME="arm64" ;;
  *)             echo "Error: Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${PLATFORM}-${ARCH_NAME}"

# ─── Resolve latest release ───────────────────────────────────────────────────
echo "turn: detecting platform... ${PLATFORM}/${ARCH_NAME}"

LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
  echo "Error: Could not determine latest release."
  exit 1
fi

# ─── Download and install core VM ─────────────────────────────────────────────
ASSET_NAME="turn-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${ASSET_NAME}"
echo "turn: downloading ${LATEST_TAG} from ${DOWNLOAD_URL}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ASSET_NAME"
tar xzf "$TMP_DIR/$ASSET_NAME" -C "$TMP_DIR"

mkdir -p "$INSTALL_DIR"
mv "$TMP_DIR/turn" "$INSTALL_DIR/turn"
chmod +x "$INSTALL_DIR/turn"

# ─── Download and install default inference provider ──────────────────────────
# The `infer` primitive delegates to an external provider binary via JSON-RPC stdio.
# Without a provider, scripts using `infer` will fail with a clear error message.
# We ship turn-provider-openai as the default; see PROVIDERS.md for alternatives.

PROVIDER_ASSET="turn-provider-openai-${TARGET}.tar.gz"
PROVIDER_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${PROVIDER_ASSET}"

echo "turn: downloading default inference provider (turn-provider-openai)..."

if curl -fsSL --head "$PROVIDER_URL" 2>/dev/null | grep -q "200 OK\|HTTP/2 200"; then
  curl -fsSL "$PROVIDER_URL" -o "$TMP_DIR/$PROVIDER_ASSET"
  tar xzf "$TMP_DIR/$PROVIDER_ASSET" -C "$TMP_DIR"
  mv "$TMP_DIR/turn-provider-openai" "$INSTALL_DIR/turn-provider-openai"
  chmod +x "$INSTALL_DIR/turn-provider-openai"
  PROVIDER_INSTALLED=1
else
  # Provider binary not yet released — skip silently, Turn still works for non-infer scripts
  PROVIDER_INSTALLED=0
fi

# ─── Update PATH ──────────────────────────────────────────────────────────────
add_to_path() {
  local profile="$1"
  if [ -f "$profile" ]; then
    if ! grep -q '.turn/bin' "$profile" 2>/dev/null; then
      echo '' >> "$profile"
      echo '# Turn language' >> "$profile"
      echo 'export PATH="$HOME/.turn/bin:$PATH"' >> "$profile"
      echo "turn: added to PATH via ${profile}"
    fi
  fi
}

case "$(basename "$SHELL")" in
  zsh)  add_to_path "$HOME/.zshrc" ;;
  bash) add_to_path "$HOME/.bashrc"
        add_to_path "$HOME/.bash_profile" ;;
  fish) mkdir -p "$HOME/.config/fish"
        echo 'set -gx PATH $HOME/.turn/bin $PATH' >> "$HOME/.config/fish/config.fish"
        echo "turn: added to PATH via config.fish" ;;
  *)    add_to_path "$HOME/.profile" ;;
esac

# ─── Verify ───────────────────────────────────────────────────────────────────
echo ""
echo "  ✓ turn installed to ${INSTALL_DIR}/turn"
echo "  ✓ version: $($INSTALL_DIR/turn --version 2>/dev/null || echo "$LATEST_TAG")"
if [ "$PROVIDER_INSTALLED" = "1" ]; then
echo "  ✓ turn-provider-openai installed (default inference driver)"
echo ""
echo "  To use the 'infer' primitive, set your LLM credentials:"
echo "    export OPENAI_API_KEY=sk-..."
echo ""
echo "  Or switch to a different provider:"
echo "    export TURN_INFER_PROVIDER=turn-provider-azure-openai"
echo "    (see https://github.com/ekizito96/Turn/blob/main/PROVIDERS.md)"
fi
echo ""
echo "  Restart your shell or run:"
echo "    export PATH=\"\$HOME/.turn/bin:\$PATH\""
echo ""
echo "  Then try:"
echo "    turn --version"
echo ""
