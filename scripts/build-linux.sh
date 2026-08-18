#!/usr/bin/env bash
set -e

echo "========================================"
echo "    Building DSH-UI for Linux (DEB/AppImage) "
echo "========================================"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/4] Checking Linux dependencies..."
if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libssl-dev libayatana-appindicator3-dev librsvg2-dev
fi

echo "[2/4] Installing dependencies..."
npm install

echo "[3/4] Generating app icons from approved whale artwork..."
npx tauri icon assets/icon-master.svg

echo "[4/4] Building Linux packages..."
npx tauri build

echo "BUILD SUCCESS! Output located in src-tauri/target/release/bundle/"
