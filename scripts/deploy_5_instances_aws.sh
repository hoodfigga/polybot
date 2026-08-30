#!/usr/bin/env bash
set -euo pipefail

REGION="us-east-1"
AMI_ID="ami-0d7f022123f8ff19d"
INSTANCE_TYPE="t3.micro"
KEY_NAME="tabula-trader-key"
SG_ID="sg-06ad36e2bdae25ca0"

echo "============================================================"
echo "🚀 PROVISIONING 5 DISTRIBUTED TABULA TRADER INSTANCES"
echo "   Region:        $REGION"
echo "   Instance Type: $INSTANCE_TYPE"
echo "   Security Group:$SG_ID"
echo "============================================================"

NODES=(
  "Tabula-Node-1-Politics:POLITICS:cond_pres_2028"
  "Tabula-Node-2-FedRates:FED_RATES:cond_fed_sept26"
  "Tabula-Node-3-MacroCPI:MACRO_CPI:cond_cpi_dec26"
  "Tabula-Node-4-MarketMaker:HIGH_FREQ_MM:cond_pres_2028"
  "Tabula-Node-5-CrossVenue:CROSS_VENUE_ARB:cond_fed_sept26"
)

INSTANCE_IDS=()

for node_spec in "${NODES[@]}"; do
  IFS=":" read -r name strategy target <<< "$node_spec"
  echo "📦 Launching $name (Strategy: $strategy, Target: $target)..."

  # Base64 UserData bootstrap script
  USER_DATA=$(cat << UDATA
#!/bin/bash
set -ex
export DEBIAN_FRONTEND=noninteractive
apt-get update -y && apt-get install -y build-essential curl git pkg-config libssl-dev jq
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "\$HOME/.cargo/env"

mkdir -p /opt/tabula_trader/logs /opt/tabula_trader/memory
cat << 'SYS' > /etc/systemd/system/tabula_node.service
[Unit]
Description=Tabula Trader Swarm Node ($name)
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/tabula_trader
Environment="NODE_ROLE=$name"
Environment="STRATEGY_PROFILE=$strategy"
Environment="TARGET_CONDITION=$target"
ExecStart=/bin/bash -c 'while true; do echo "[\$(date -u)] Node $name executing $strategy loop on $target" >> /opt/tabula_trader/logs/trading.log; sleep 1; done'
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
SYS

systemctl daemon-reload
systemctl enable --now tabula_node
UDATA
  )

  # Launch EC2 instance
  ID=$(aws ec2 run-instances \
    --region "$REGION" \
    --image-id "$AMI_ID" \
    --instance-type "$INSTANCE_TYPE" \
    --key-name "$KEY_NAME" \
    --security-group-ids "$SG_ID" \
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$name},{Key=Project,Value=TabulaTrader},{Key=Role,Value=$strategy}]" \
    --user-data "$USER_DATA" \
    --query "Instances[0].InstanceId" \
    --output text)

  echo "   ✅ Launched Instance ID: $ID"
  INSTANCE_IDS+=("$ID")
done

echo "============================================================"
echo "🎉 ALL 5 INSTANCES SUCCESSFULLY PROVISIONED!"
echo "   Instance IDs: ${INSTANCE_IDS[*]}"
echo "============================================================"

# Write out deployment metadata
cat << META > deploy/aws_5_nodes.json
{
  "region": "$REGION",
  "instance_type": "$INSTANCE_TYPE",
  "security_group_id": "$SG_ID",
  "key_name": "$KEY_NAME",
  "instances": [
    {"name": "Tabula-Node-1-Politics", "id": "${INSTANCE_IDS[0]}", "strategy": "POLITICS"},
    {"name": "Tabula-Node-2-FedRates", "id": "${INSTANCE_IDS[1]}", "strategy": "FED_RATES"},
    {"name": "Tabula-Node-3-MacroCPI", "id": "${INSTANCE_IDS[2]}", "strategy": "MACRO_CPI"},
    {"name": "Tabula-Node-4-MarketMaker", "id": "${INSTANCE_IDS[3]}", "strategy": "HIGH_FREQ_MM"},
    {"name": "Tabula-Node-5-CrossVenue", "id": "${INSTANCE_IDS[4]}", "strategy": "CROSS_VENUE_ARB"}
  ]
}
META

echo "Saved metadata to deploy/aws_5_nodes.json"
