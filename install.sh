#!/bin/sh
# Turn Language Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/ekizito96/Turn/main/install.sh | bash
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
ASSET_NAME="turn-${TARGET}.tar.gz"

# ─── Resolve latest release ───────────────────────────────────────────────────
echo "turn: detecting platform... ${PLATFORM}/${ARCH_NAME}"

LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
  echo "Error: Could not determine latest release."
  exit 1
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${ASSET_NAME}"
echo "turn: downloading ${LATEST_TAG} from ${DOWNLOAD_URL}"

# ─── Download and install ─────────────────────────────────────────────────────
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ASSET_NAME"
tar xzf "$TMP_DIR/$ASSET_NAME" -C "$TMP_DIR"

mkdir -p "$INSTALL_DIR"
mv "$TMP_DIR/turn" "$INSTALL_DIR/turn"
chmod +x "$INSTALL_DIR/turn"

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
echo ""
echo "  Restart your shell or run:"
echo "    export PATH=\"\$HOME/.turn/bin:\$PATH\""
echo ""
echo "  Then try:"
echo "    turn --version"
echo ""
