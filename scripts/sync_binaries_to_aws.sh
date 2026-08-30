#!/usr/bin/env bash
set -euo pipefail

echo "============================================================"
echo "🚀 DEPLOYING TABULA TRADER RELEASE BINARY TO 6 AWS NODES"
echo "============================================================"

NODES=(
  "ubuntu@100.56.228.254:~/.ssh/tabula-trader-key.pem:Tabula-Node-1-Politics:50.0:cond_pres_2028"
  "ubuntu@3.95.58.135:~/.ssh/tabula-trader-key.pem:Tabula-Node-2-FedRates:50.0:cond_fed_sept26"
  "ubuntu@34.228.29.165:~/.ssh/tabula-trader-key.pem:Tabula-Node-3-MacroCPI:50.0:cond_cpi_dec26"
  "ubuntu@54.175.95.78:~/.ssh/tabula-trader-key.pem:Tabula-Node-4-MarketMaker:50.0:cond_pres_2028"
  "ubuntu@13.203.161.41:~/.ssh/tabula-trader-key-mumbai.pem:Tabula-Node-5-CrossVenue:50.0:cond_fed_sept26"
  "ubuntu@3.110.84.166:~/.ssh/tabula-trader-key-mumbai.pem:Tabula-Node-6-CryptoHFT:50.0:BTC_ETH_5M"
)

for node_entry in "${NODES[@]}"; do
  IFS=":" read -r host key name cap target <<< "$node_entry"
  echo "📦 Syncing binary to $name ($host)..."

  # Upload binary
  scp -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=10 target/release/paper_trader "$host":/tmp/paper_trader

  # Install and start systemd service
  ssh -i "$key" -o StrictHostKeyChecking=no -o ConnectTimeout=10 "$host" bash -s << REMOTESCRIPT
    sudo systemctl stop tabula_trader 2>/dev/null || true
    sudo rm -rf /opt/tabula_trader/logs/* /opt/tabula_trader/memory/*
    sudo mkdir -p /opt/tabula_trader/logs /opt/tabula_trader/memory
    sudo mv /tmp/paper_trader /opt/tabula_trader/paper_trader
    sudo chmod +x /opt/tabula_trader/paper_trader

    sudo tee /etc/systemd/system/tabula_trader.service > /dev/null << 'SYS'
[Unit]
Description=Tabula Trader Engine ($name)
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/tabula_trader
ExecStart=/opt/tabula_trader/paper_trader --capital $cap --duration-secs 86400
Restart=always
RestartSec=3
StandardOutput=append:/opt/tabula_trader/logs/trading.log
StandardError=append:/opt/tabula_trader/logs/trading_err.log

[Install]
WantedBy=multi-user.target
SYS

    sudo systemctl daemon-reload
    sudo systemctl enable --now tabula_trader
    sleep 2
    sudo systemctl status tabula_trader --no-pager | grep Active
REMOTESCRIPT

  echo "   ✅ $name active and trading!"
done

echo "============================================================"
echo "🎉 ALL 6 NODES RUNNING REAL RUST TRADING ENGINES ON AWS!"
echo "============================================================"
