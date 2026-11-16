#!/bin/sh
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

SRC="$ROOT_DIR/src/mtk-fb-blank.c"
OUT="$ROOT_DIR/overlay/usr/local/bin/mtk-fb-blank"

CC="${CROSS_COMPILE}gcc"
if ! command -v "$CC" >/dev/null 2>&1; then
	if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
		CC="aarch64-linux-gnu-gcc"
	elif command -v gcc >/dev/null 2>&1; then
		CC="gcc"
	else
		echo "Error: C compiler not found ($CC)." >&2
		exit 1
	fi
fi

echo "Compiling $SRC with $CC..."
mkdir -p "$ROOT_DIR/overlay/usr/local/bin"
"$CC" -O2 -Wall -s -o "$OUT" "$SRC"
chmod 755 "$OUT"
echo "Built: $OUT"
