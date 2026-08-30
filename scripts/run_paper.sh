#!/usr/bin/env bash
set -euo pipefail

echo "============================================================"
echo "🧪 Starting Tabula Trader Zero-Risk Paper Trading Sandbox"
echo "============================================================"

cargo run --bin paper_trader -- "$@"
