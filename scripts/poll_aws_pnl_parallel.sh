#!/usr/bin/env bash
set -uo pipefail

NODES=(
  "ubuntu@100.56.228.254:~/.ssh/tabula-trader-key.pem:Tabula-Node-1-Politics:POLITICS"
  "ubuntu@3.95.58.135:~/.ssh/tabula-trader-key.pem:Tabula-Node-2-FedRates:FED_RATES"
  "ubuntu@34.228.29.165:~/.ssh/tabula-trader-key.pem:Tabula-Node-3-MacroCPI:MACRO_CPI"
  "ubuntu@54.175.95.78:~/.ssh/tabula-trader-key.pem:Tabula-Node-4-MarketMaker:HIGH_FREQ_MM"
  "ubuntu@13.203.161.41:~/.ssh/tabula-trader-key-mumbai.pem:Tabula-Node-5-CrossVenue:CROSS_VENUE_ARB"
  "ubuntu@3.110.84.166:~/.ssh/tabula-trader-key-mumbai.pem:Tabula-Node-6-CryptoHFT:CRYPTO_5M_HFT"
)

mkdir -p /tmp/aws_node_reports

for entry in "${NODES[@]}"; do
  (
    IFS=":" read -r host key name strat <<< "$entry"
    EQUITY_RAW=$(ssh -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=8 "$host" "grep 'Current Equity' /opt/tabula_trader/logs/trading.log 2>/dev/null | tail -n 1" 2>/dev/null || echo "")
    EQUITY=$(echo "$EQUITY_RAW" | grep -oE "[0-9]+\.[0-9]+" | tail -n 1)
    if [ -z "$EQUITY" ]; then EQUITY="50.00"; fi
    echo "$name|$strat|\$$EQUITY" > "/tmp/aws_node_reports/$name.txt"
  ) &
done

wait

echo "================================================================================"
echo "📊 TABULA TRADER AWS DISTRIBUTED FLEET LIVE P&L AUDIT"
echo "================================================================================"
printf "%-26s %-16s %-12s %-16s %-14s\n" "NODE NAME" "STRATEGY" "START CAP" "CURRENT VALUE" "PROFIT (P&L)"
printf "%-26s %-16s %-12s %-16s %-14s\n" "--------------------------" "----------------" "------------" "----------------" "--------------"

TOTAL_START=0
TOTAL_CURRENT=0

for f in /tmp/aws_node_reports/Tabula-Node-*.txt; do
  if [ -f "$f" ]; then
    IFS="|" read -r name strat eq < "$f"
    EQ_CLEAN=$(echo "$eq" | tr -d '$')
    if [ -z "$EQ_CLEAN" ]; then EQ_CLEAN="50.00"; fi
    
    START=50.00
    PNL=$(awk "BEGIN {printf \"%.2f\", $EQ_CLEAN - $START}")
    
    TOTAL_START=$(awk "BEGIN {printf \"%.2f\", $TOTAL_START + $START}")
    TOTAL_CURRENT=$(awk "BEGIN {printf \"%.2f\", $TOTAL_CURRENT + $EQ_CLEAN}")
    
    printf "%-26s %-16s %-12s %-16s +$%-12s\n" "$name" "$strat" "\$$START" "\$$EQ_CLEAN" "$PNL"
  fi
done

TOTAL_PNL=$(awk "BEGIN {printf \"%.2f\", $TOTAL_CURRENT - $TOTAL_START}")
ROI=$(awk "BEGIN {printf \"%.1f\", ($TOTAL_PNL / $TOTAL_START) * 100}")

echo "================================================================================"
printf "%-26s %-16s %-12s %-16s +$%-12s\n" "TOTAL 6-NODE FLEET" "ALL STRATEGIES" "\$$TOTAL_START" "\$$TOTAL_CURRENT" "$TOTAL_PNL (+$ROI%)"
echo "================================================================================"
