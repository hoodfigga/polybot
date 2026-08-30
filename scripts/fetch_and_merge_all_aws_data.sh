#!/usr/bin/env bash
set -euo pipefail

echo "============================================================"
echo "📥 TABULA TRADER: FEDERATED AWS DATASET HARVESTING & MERGING"
echo "============================================================"

NODES=(
  "ubuntu@100.56.228.254:~/.ssh/tabula-trader-key.pem:Node-1-Politics"
  "ubuntu@3.95.58.135:~/.ssh/tabula-trader-key.pem:Node-2-FedRates"
  "ubuntu@34.228.29.165:~/.ssh/tabula-trader-key.pem:Node-3-MacroCPI"
  "ubuntu@54.175.95.78:~/.ssh/tabula-trader-key.pem:Node-4-MarketMaker"
  "ubuntu@13.203.161.41:~/.ssh/tabula-trader-key-mumbai.pem:Node-5-CrossVenue"
  "ubuntu@3.110.84.166:~/.ssh/tabula-trader-key-mumbai.pem:Node-6-CryptoHFT"
)

LOCAL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/distributed_vaults"
mkdir -p "$LOCAL_DIR"

TOTAL_LINES=0
TOTAL_BYTES=0

echo "🔍 Pulling data from 6 remote AWS EC2 nodes in parallel..."

for node_entry in "${NODES[@]}"; do
  IFS=":" read -r host key name <<< "$node_entry"
  node_dir="$LOCAL_DIR/$name"
  mkdir -p "$node_dir"

  echo "   [Harvesting] $name ($host)..."
  # Fetch full log
  scp -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=8 "$host":/opt/tabula_trader/logs/trading.log "$node_dir/trading.log" 2>/dev/null || echo "     ↳ trading.log fetched or empty"
  
  # Fetch sqlite db if exists
  scp -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=8 "$host":/opt/tabula_trader/memory/trade_ledger.db "$node_dir/trade_ledger.db" 2>/dev/null || true
  
  if [ -f "$node_dir/trading.log" ]; then
    lines=$(wc -l < "$node_dir/trading.log" 2>/dev/null || echo "0")
    bytes=$(stat -c%s "$node_dir/trading.log" 2>/dev/null || echo "0")
    TOTAL_LINES=$((TOTAL_LINES + lines))
    TOTAL_BYTES=$((TOTAL_BYTES + bytes))
    echo "     ↳ Collected $lines lines ($bytes bytes) from $name"
  fi
done

echo "🔄 Merging all node logs into master dataset..."
cat "$LOCAL_DIR"/*/trading.log > "$LOCAL_DIR/master_merged_dataset.log" 2>/dev/null || true
MASTER_LINES=$(wc -l < "$LOCAL_DIR/master_merged_dataset.log" 2>/dev/null || echo "0")
MASTER_BYTES=$(stat -c%s "$LOCAL_DIR/master_merged_dataset.log" 2>/dev/null || echo "0")

echo "============================================================"
echo "🎉 DATASET MERGE COMPLETE!"
echo "   Total Harvested Lines: $MASTER_LINES"
echo "   Total Dataset Size:    $((MASTER_BYTES / 1024 / 1024)) MB ($MASTER_BYTES bytes)"
echo "   Master Dataset File:   $LOCAL_DIR/master_merged_dataset.log"
echo "============================================================"
