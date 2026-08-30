#!/usr/bin/env bash
set -euo pipefail

echo "============================================================"
echo "📈 Starting Tabula Trader Strategy Backtest Replay"
echo "============================================================"

cargo run --bin backtest_runner -- "$@"
