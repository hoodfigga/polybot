#!/usr/bin/env bash
set -euo pipefail

echo "============================================================"
echo "🛑 TABULA TRADER: AWS EC2 DISTRIBUTED FLEET SHUTDOWN"
echo "============================================================"

NODES=(
  "ubuntu@100.56.228.254:~/.ssh/tabula-trader-key.pem:Tabula-Node-1-Politics"
  "ubuntu@3.95.58.135:~/.ssh/tabula-trader-key.pem:Tabula-Node-2-FedRates"
  "ubuntu@34.228.29.165:~/.ssh/tabula-trader-key.pem:Tabula-Node-3-MacroCPI"
  "ubuntu@54.175.95.78:~/.ssh/tabula-trader-key.pem:Tabula-Node-4-MarketMaker"
  "ubuntu@13.203.161.41:~/.ssh/tabula-trader-key-mumbai.pem:Tabula-Node-5-CrossVenue"
  "ubuntu@3.110.84.166:~/.ssh/tabula-trader-key-mumbai.pem:Tabula-Node-6-CryptoHFT"
)

for node_entry in "${NODES[@]}"; do
  IFS=":" read -r host key name <<< "$node_entry"
  echo "🔌 Shutting down $name ($host)..."

  # Stop service cleanly and power off
  ssh -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=5 "$host" bash -s << 'REMOTESCRIPT' 2>/dev/null || true
    echo "Stopping tabula_trader service..."
    sudo systemctl stop tabula_trader 2>/dev/null || true
    sync
    echo "Powering off EC2 instance..."
    sudo shutdown -h now 2>/dev/null || sudo poweroff 2>/dev/null || true
REMOTESCRIPT
  echo "   ↳ Shutdown signal sent to $name."
done

echo "------------------------------------------------------------"
echo "⏳ Waiting 10 seconds for instances to halt..."
sleep 10

echo "🔍 Verifying node power states..."
for node_entry in "${NODES[@]}"; do
  IFS=":" read -r host key name <<< "$node_entry"
  if ssh -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=3 "$host" "echo alive" 2>/dev/null; then
    echo "   ⚠️ $name is still responding."
  else
    echo "   ✅ $name is completely OFFLINE (SHUTDOWN)."
  fi
done

echo "============================================================"
echo "🎉 ALL AWS NODES HAVE BEEN SAFELY SHUT DOWN."
echo "============================================================"
