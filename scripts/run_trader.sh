#!/usr/bin/env bash
set -euo pipefail

echo "============================================================"
echo "🚀 Starting Tabula Trader Production Daemon"
echo "============================================================"

export RUST_LOG="${RUST_LOG:-info}"
export ODDSQ_API_KEY="${ODDSQ_API_KEY:-demo_key}"
export ODDSQ_WEBHOOK_SECRET="${ODDSQ_WEBHOOK_SECRET:-tabula_trader_default_secret}"

# Run trading_node binary in release mode
cargo run --release --bin trading_node -- "$@"
