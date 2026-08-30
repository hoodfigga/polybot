#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$SCRIPT_DIR"

echo "============================================================"
echo "🔐 TABULA TRADER: AUTOMATIC POLYMARKET CLOB ONBOARDING"
echo "============================================================"

./target/release/generate_polymarket_credentials "$@"
