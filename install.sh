#!/usr/bin/env sh
set -eu

REPOSITORY="tu-tu-op/GitSama"
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
USER_HOME=$(printenv HOME 2>/dev/null || true)
if [ -z "$USER_HOME" ]; then
  echo "Could not find your home directory. Set HOME and run install.sh again." >&2
  exit 1
fi
INSTALL_ROOT="$USER_HOME/.gitsama"
BIN_DIR="$INSTALL_ROOT/bin"
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$SCRIPT_DIR/Cargo.toml" | head -n 1)
if [ -z "$VERSION" ]; then VERSION=0.1.0; fi

if ! command -v git >/dev/null 2>&1; then
  echo "GitSama needs Git 2.54 or newer. Git was not found." >&2
  exit 1
fi
GIT_VERSION=$(git --version | awk '{print $3}')
GIT_MAJOR=$(printf '%s' "$GIT_VERSION" | cut -d. -f1)
GIT_MINOR=$(printf '%s' "$GIT_VERSION" | cut -d. -f2)
case "$GIT_MAJOR" in ''|*[!0-9]*) GIT_MAJOR=0 ;; esac
case "$GIT_MINOR" in ''|*[!0-9]*) GIT_MINOR=0 ;; esac
if [ "$GIT_MAJOR" -lt 2 ] || { [ "$GIT_MAJOR" -eq 2 ] && [ "$GIT_MINOR" -lt 54 ]; }; then
  echo "GitSama needs Git 2.54 or newer." >&2
  echo "Detected: Git $GIT_VERSION" >&2
  echo "Upgrade Git and run ./install.sh again." >&2
  exit 1
fi

SOURCE=""
if [ -x "$SCRIPT_DIR/target/release/gitsama" ]; then
  SOURCE="$SCRIPT_DIR/target/release/gitsama"
elif command -v cargo >/dev/null 2>&1; then
  echo "Building GitSama from source..."
  cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
  SOURCE="$SCRIPT_DIR/target/release/gitsama"
else
  OS_NAME=$(uname -s)
  ARCH_NAME=$(uname -m)
  case "$OS_NAME:$ARCH_NAME" in
    Linux:x86_64|Linux:amd64) ASSET="gitsama-v$VERSION-linux-x86_64.tar.gz" ;;
    Darwin:x86_64|Darwin:amd64) ASSET="gitsama-v$VERSION-macos-x86_64.tar.gz" ;;
    Darwin:arm64|Darwin:aarch64) ASSET="gitsama-v$VERSION-macos-aarch64.tar.gz" ;;
    *)
      echo "No release asset is available for $OS_NAME/$ARCH_NAME." >&2
      echo "Install Rust and Cargo, then run install.sh again." >&2
      exit 1
      ;;
  esac
  TEMP_DIR=$(mktemp -d)
  trap 'rm -rf "$TEMP_DIR"' EXIT HUP INT TERM
  ARCHIVE="$TEMP_DIR/$ASSET"
  CHECKSUM="$ARCHIVE.sha256"
  URL="https://github.com/$REPOSITORY/releases/download/v$VERSION/$ASSET"
  echo "Downloading $URL"
  if command -v curl >/dev/null 2>&1; then
    curl --fail --location --silent --show-error "$URL" --output "$ARCHIVE"
    curl --fail --location --silent --show-error "$URL.sha256" --output "$CHECKSUM"
  elif command -v wget >/dev/null 2>&1; then
    wget --quiet --output-document "$ARCHIVE" "$URL"
    wget --quiet --output-document "$CHECKSUM" "$URL.sha256"
  else
    echo "Install Rust/Cargo or install curl/wget to download the release." >&2
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$TEMP_DIR" && sha256sum --check "$CHECKSUM")
  elif command -v shasum >/dev/null 2>&1; then
    (cd "$TEMP_DIR" && shasum -a 256 --check "$CHECKSUM")
  else
    echo "Cannot verify the release checksum; install sha256sum or shasum." >&2
    exit 1
  fi
  mkdir -p "$TEMP_DIR/unpacked"
  tar -xzf "$ARCHIVE" -C "$TEMP_DIR/unpacked"
  SOURCE="$TEMP_DIR/unpacked/gitsama"
fi

if [ ! -f "$SOURCE" ]; then
  echo "GitSama binary was not produced. Check the build output and try again." >&2
  exit 1
fi

mkdir -p "$BIN_DIR"
cp "$SOURCE" "$BIN_DIR/gitsama"
chmod 755 "$BIN_DIR/gitsama"

PROFILE=$(printenv GITSAMA_SHELL_PROFILE 2>/dev/null || true)
if [ -z "$PROFILE" ]; then
  SHELL_NAME=$(printenv SHELL 2>/dev/null || true)
  case $(basename "${SHELL_NAME:-sh}") in
    zsh) PROFILE="$USER_HOME/.zshrc" ;;
    *) PROFILE="$USER_HOME/.profile" ;;
  esac
fi
if [ ! -f "$PROFILE" ] || ! grep -Fq "# GitSama PATH" "$PROFILE"; then
  {
    echo ""
    echo "# GitSama PATH"
    echo "export PATH=\"$BIN_DIR:\$PATH\""
  } >> "$PROFILE"
  echo "Added $BIN_DIR to $PROFILE"
fi

GITSAMA_NONINTERACTIVE=1 "$BIN_DIR/gitsama" setup
echo ""
echo "GitSama installed at $BIN_DIR/gitsama."
echo "Open a new shell if gitsama is not immediately on PATH."
