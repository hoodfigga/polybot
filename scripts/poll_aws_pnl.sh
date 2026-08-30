#!/usr/bin/env bash
set -uo pipefail

echo "================================================================================"
echo "📊 TABULA TRADER AWS DISTRIBUTED FLEET LIVE P&L AUDIT"
echo "================================================================================"

NODES=(
  "ubuntu@100.56.228.254:~/.ssh/tabula-trader-key.pem:Tabula-Node-1-Politics:POLITICS"
  "ubuntu@3.95.58.135:~/.ssh/tabula-trader-key.pem:Tabula-Node-2-FedRates:FED_RATES"
  "ubuntu@34.228.29.165:~/.ssh/tabula-trader-key.pem:Tabula-Node-3-MacroCPI:MACRO_CPI"
  "ubuntu@54.175.95.78:~/.ssh/tabula-trader-key.pem:Tabula-Node-4-MarketMaker:HIGH_FREQ_MM"
  "ubuntu@13.203.161.41:~/.ssh/tabula-trader-key-mumbai.pem:Tabula-Node-5-CrossVenue:CROSS_VENUE_ARB"
  "ubuntu@3.110.84.166:~/.ssh/tabula-trader-key-mumbai.pem:Tabula-Node-6-CryptoHFT:CRYPTO_5M_HFT"
)

TOTAL_START=0
TOTAL_CURRENT=0
TOTAL_PNL=0

printf "%-26s %-15s %-12s %-14s %-12s %-8s\n" "NODE NAME" "STRATEGY" "START CAP" "CURRENT VALUE" "PROFIT (P&L)" "STATUS"
printf "%-26s %-15s %-12s %-14s %-12s %-8s\n" "--------------------------" "---------------" "------------" "--------------" "------------" "--------"

for entry in "${NODES[@]}"; do
  IFS=":" read -r host key name strat <<< "$entry"
  
  # Fetch last line of trading log
  LAST_LINE=$(ssh -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=5 "$host" "tail -n 25 /opt/tabula_trader/logs/trading.log 2>/dev/null | grep -E 'Tick|Capital|Session|PnL|Finished' | tail -n 1" 2>/dev/null || echo "")
  SVC_STATE=$(ssh -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=5 "$host" "systemctl is-active tabula_trader 2>/dev/null" 2>/dev/null || echo "offline")

  # Parse capital and pnl if available from log
  # Log format typically: "... Capital: $XXX.XX | PnL: +$YY.YY ..."
  START_CAP="50.00"
  
  echo "--- [$name ($strat)] State: $SVC_STATE ---"
  ssh -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=5 "$host" "tail -n 6 /opt/tabula_trader/logs/trading.log 2>/dev/null" 2>/dev/null || echo "No logs"
  echo ""
done

