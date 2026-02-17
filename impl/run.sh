#!/usr/bin/env bash
# Run Turn: builds and executes. Ensures cargo is on PATH.
set -e

cd "$(dirname "$0")"

# install-rust can run without cargo
if [ "${1:-}" = "install-rust" ]; then
  echo "Installing Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  echo ""
  echo "Done! Run: source \$HOME/.cargo/env"
  echo "Then try: ./run.sh test"
  exit 0
fi

# Add cargo to PATH if not found
if ! command -v cargo &>/dev/null; then
  if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
  elif [ -x "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
fi

if ! command -v cargo &>/dev/null; then
  echo "Error: cargo not found."
  echo ""
  echo "Install Rust with: ./run.sh install-rust"
  echo "Or manually: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  echo ""
  echo "Then run: source \$HOME/.cargo/env"
  exit 1
fi

case "${1:-}" in
  test)
    cargo test
    ;;
  build)
    cargo build --release
    ;;
  hello)
    cargo run --quiet -- run tests/hello_turn.turn
    ;;
  examples)
    echo "Running examples..."
    for f in examples/*.turn; do
      echo "--- $f ---"
      cargo run --quiet -- run "$f" 2>/dev/null || true
    done
    ;;
  install-rust)
    echo "Rust should be installed. Run: source \$HOME/.cargo/env"
    echo "Then: ./run.sh test"
    ;;
  *)
    echo "Usage: ./run.sh [test|build|hello|examples|install-rust]"
    echo ""
    echo "  test         - run all tests"
    echo "  build        - build release binary"
    echo "  hello        - run hello_turn.turn (prints 'Hello')"
    echo "  examples     - run all examples"
    echo "  install-rust - install Rust (if cargo not found)"
    echo ""
    echo "Or: cargo run -- run <file.turn>"
    exit 1
    ;;
esac
