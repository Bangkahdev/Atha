#!/bin/bash

set -e

echo "[atha] Installing Atha..."

BINARY_URL="https://github.com/Bangkah/Atha/releases/latest/download/atha-x86_64-linux"
TMP_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/atha-installer"

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "[atha] Missing dependency: $1"
        exit 1
    fi
}

require_cmd sudo

have_cmd() {
    command -v "$1" >/dev/null 2>&1
}

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

if have_cmd curl; then
    echo "[atha] Downloading binary via curl..."
    curl -fsSL "$BINARY_URL" -o "$TMP_DIR/atha"
elif have_cmd wget; then
    echo "[atha] Downloading binary via wget..."
    wget -qO "$TMP_DIR/atha" "$BINARY_URL"
else
    echo "[atha] Missing dependency: install one of curl or wget"
    exit 1
fi

sudo install -Dm755 "$TMP_DIR/atha" /usr/bin/atha

echo "[atha] Installation complete!"
