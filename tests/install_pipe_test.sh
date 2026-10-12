#!/usr/bin/env sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TEMP_ROOT=$(mktemp -d)
trap 'rm -rf "$TEMP_ROOT"' EXIT HUP INT TERM
FIXTURES="$TEMP_ROOT/fixtures"
BIN="$TEMP_ROOT/mock-bin"
HOME_DIR="$TEMP_ROOT/home \$GITSAMA_PROFILE_INJECTED \`printf injected\`"
WORK="$TEMP_ROOT/work"
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)
ASSET="gitsama-v$VERSION-linux-x86_64.tar.gz"
mkdir -p "$FIXTURES" "$BIN" "$HOME_DIR" "$WORK/target/release"

cat > "$FIXTURES/gitsama" <<'EOF'
#!/usr/bin/env sh
printf release > "$GITSAMA_TEST_MARKER"
EOF
chmod 755 "$FIXTURES/gitsama"
tar -czf "$FIXTURES/$ASSET" -C "$FIXTURES" gitsama
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$FIXTURES" && sha256sum "$ASSET" > "$ASSET.sha256")
else
  (cd "$FIXTURES" && shasum -a 256 "$ASSET" > "$ASSET.sha256")
fi

cat > "$WORK/target/release/gitsama" <<'EOF'
#!/usr/bin/env sh
printf caller-directory > "$GITSAMA_TEST_MARKER"
EOF
chmod 755 "$WORK/target/release/gitsama"

cat > "$BIN/git" <<'EOF'
#!/usr/bin/env sh
if [ "${1:-}" = "--version" ]; then
  printf 'git version 2.54.0\n'
fi
EOF
cat > "$BIN/uname" <<'EOF'
#!/usr/bin/env sh
case "$1" in
  -s) printf 'Linux\n' ;;
  -m) printf 'x86_64\n' ;;
  *) exit 1 ;;
esac
EOF
cat > "$BIN/curl" <<'EOF'
#!/usr/bin/env sh
set -eu
OUTPUT=""
URL=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) OUTPUT=$2; shift 2 ;;
    http://*|https://*) URL=$1; shift ;;
    *) shift ;;
  esac
done
if [ -z "$OUTPUT" ] || [ -z "$URL" ]; then exit 1; fi
cp "$GITSAMA_TEST_FIXTURES/${URL##*/}" "$OUTPUT"
EOF
chmod 755 "$BIN/git" "$BIN/uname" "$BIN/curl"

(
  cd "$WORK"
  export PATH="$BIN:$PATH"
  export HOME="$HOME_DIR"
  export GITSAMA_TEST_FIXTURES="$FIXTURES"
  export GITSAMA_TEST_MARKER="$TEMP_ROOT/selected-binary"
  cat "$ROOT/install.sh" | sh
)

test "$(cat "$TEMP_ROOT/selected-binary")" = release
(
  GITSAMA_PROFILE_INJECTED=injected
  export GITSAMA_PROFILE_INJECTED
  . "$HOME_DIR/.profile"
  case "$PATH" in "$HOME_DIR/.gitsama/bin:"*) ;; *) echo "Profile path was interpreted as shell code" >&2; exit 1 ;; esac
)

printf unexpected > "$FIXTURES/extra-file"
tar -czf "$FIXTURES/$ASSET" -C "$FIXTURES" gitsama extra-file
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$FIXTURES" && sha256sum "$ASSET" > "$ASSET.sha256")
else
  (cd "$FIXTURES" && shasum -a 256 "$ASSET" > "$ASSET.sha256")
fi
if (
  cd "$WORK"
  export PATH="$BIN:$PATH" HOME="$HOME_DIR"
  export GITSAMA_TEST_FIXTURES="$FIXTURES" GITSAMA_TEST_MARKER="$TEMP_ROOT/selected-binary"
  cat "$ROOT/install.sh" | sh
); then
  echo "Installer accepted an archive with unexpected entries" >&2
  exit 1
fi
