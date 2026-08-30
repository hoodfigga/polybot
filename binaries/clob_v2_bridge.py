#!/usr/bin/env python3
"""
Polymarket CLOB V2 Execution Bridge (Thread-Safe with Capital Guard)
Handles EIP-712 Signing and Level-2 Authenticated Order Submissions
via official Polymarket py_clob_client_v2 SDK with SignatureType 3 (Deposit Wallet).
"""

import os
import json
import asyncio
import urllib.request
from aiohttp import web
from py_clob_client_v2.client import ClobClient
from py_clob_client_v2.clob_types import ApiCreds, OrderArgsV2, OrderType, OpenOrderParams

def load_env():
    env_file = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), ".secrets/trading.env")
    if os.path.exists(env_file):
        with open(env_file, "r") as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, v = line.split("=", 1)
                    v = v.strip().strip('"').strip("'")
                    os.environ.setdefault(k, v)

load_env()

API_KEY = os.environ.get("POLY_API_KEY", "6b6d6a28-112a-9c24-a730-f6baa987716b")
API_SECRET = os.environ.get("POLY_SECRET", "06tjqYwQbnep2JjIxf0R5MsMlx6rEwI1854DIoh1ACo=")
API_PASSPHRASE = os.environ.get("POLY_PASSPHRASE", "04cf1e5640d817d497133ae8ea39c6a44547e2631ebd01c508548f68f999fb2e")
PRIVATE_KEY = os.environ.get("POLYMARKET_PRIVATE_KEY", "0x4d22ae543863b449e6e8aeb095eb25b63aae2935e424d9e4f991ec7cd7658ba6")
FUNDER_PROXY = os.environ.get("POLYMARKET_PROXY_ADDRESS", "0x6674C3dC820B3A9dED849d02C8D7437783EA3Ead")
RPC_URL = os.environ.get("POLYGON_RPC_URL", "https://polygon-bor-rpc.publicnode.com")

creds = ApiCreds(api_key=API_KEY, api_secret=API_SECRET, api_passphrase=API_PASSPHRASE)
client = ClobClient(
    "https://clob.polymarket.com",
    key=PRIVATE_KEY,
    chain_id=137,
    creds=creds,
    signature_type=3,  # POLY_1271 / Deposit Wallet
    funder=FUNDER_PROXY
)

order_lock = asyncio.Lock()

def get_onchain_pusd_balance():
    try:
        clean_addr = FUNDER_PROXY.lower().replace("0x", "")
        calldata = f"0x70a08231000000000000000000000000{clean_addr}"
        body = json.dumps({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{"to": "0xc011a7e12a19f7b1f670d46f03b03f3342e82dfb", "data": calldata}, "latest"],
            "id": 1
        }).encode("utf-8")
        req = urllib.request.Request(RPC_URL, data=body, headers={"Content-Type": "application/json", "User-Agent": "Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=3) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            if "result" in data:
                raw = int(data["result"], 16)
                return raw / 1e6
    except Exception as e:
        print("Balance check error:", e)
    return 1.50

routes = web.RouteTableDef()

@routes.get("/health")
async def health(request):
    bal = get_onchain_pusd_balance()
    return web.json_response({"status": "ok", "address": FUNDER_PROXY, "available_pusd": bal, "signature_type": 3})

@routes.post("/post_order")
async def post_order(request):
    async with order_lock:
        try:
            data = await request.json()
            token_id = str(data["token_id"])
            price = float(data["price"])
            size = float(data.get("size", 5.0))
            side = str(data.get("side", "BUY")).upper()
            
            # Capital guard: check available balance
            avail_bal = get_onchain_pusd_balance()
            req_notional = price * size
            
            if side == "BUY":
                if avail_bal < 0.20:
                    return web.json_response({"success": False, "error": f"Available capital depleted (${avail_bal:.2f})" }, status=400)
                
                # If requested order exceeds available balance, adjust size down to fit within capital
                if req_notional > avail_bal:
                    # Polymarket requires minimum 5 shares and at least $1.00 notional
                    max_affordable_size = max(5.0, round((avail_bal * 0.95) / price, 1))
                    if max_affordable_size * price > avail_bal:
                        # Price is too high for current balance, wait for balance or lower-priced token
                        return web.json_response({"success": False, "error": f"Order requires ${req_notional:.2f}, but only ${avail_bal:.2f} available" }, status=400)
                    size = max_affordable_size
            
            order_args = OrderArgsV2(token_id=token_id, price=price, size=size, side=side)
            signed_order = client.create_order(order_args)
            resp = client.post_order(signed_order, OrderType.GTC)
            print(f"✅ Order Executed: {side} {size} shares @ ${price:.3f} | ID: {resp.get('orderID') or resp.get('status')}")
            return web.json_response({"success": True, "result": resp})
        except Exception as e:
            return web.json_response({"success": False, "error": str(e)}, status=400)

@routes.get("/open_orders")
async def open_orders(request):
    try:
        orders = client.get_open_orders(OpenOrderParams())
        return web.json_response({"success": True, "orders": orders})
    except Exception as e:
        return web.json_response({"success": False, "error": str(e)}, status=500)

@routes.post("/cancel_all")
async def cancel_all(request):
    try:
        resp = client.cancel_all()
        return web.json_response({"success": True, "result": resp})
    except Exception as e:
        return web.json_response({"success": False, "error": str(e)}, status=500)

app = web.Application()
app.add_routes(routes)

if __name__ == "__main__":
    port = int(os.environ.get("BRIDGE_PORT", 9006))
    print(f"🚀 Polymarket CLOB V2 Bridge running on port {port} (Proxy: {FUNDER_PROXY})")
    web.run_app(app, host="127.0.0.1", port=port)
