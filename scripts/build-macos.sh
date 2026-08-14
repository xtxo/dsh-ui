#!/usr/bin/env bash
set -e

echo "========================================"
echo "    Building DSH-UI for macOS (DMG)     "
echo "========================================"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/2] Installing dependencies..."
npm install

echo "[2/2] Building macOS Universal DMG via Tauri CLI..."
npx tauri build --target universal-apple-darwin

echo "BUILD SUCCESS! Output located in src-tauri/target/universal-apple-darwin/release/bundle/dmg/"
