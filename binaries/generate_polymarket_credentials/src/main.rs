use clap::Parser;
use ethers::signers::{LocalWallet, Signer};
use ethers::types::Address;
use net_polymarket::signer::PolymarketSigner;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(name = "generate_polymarket_credentials")]
#[command(about = "Automatic Level-2 CLOB API Credential Generator for Polymarket")]
struct Args {
    #[arg(
        short = 'k',
        long = "private-key",
        help = "Polygon EVM wallet private key (hex format)"
    )]
    private_key: Option<String>,

    #[arg(
        long = "env-file",
        default_value = ".secrets/trading.env",
        help = "Target output env file path"
    )]
    env_file: String,

    #[arg(
        long = "clob-url",
        default_value = "https://clob.polymarket.com",
        help = "Polymarket CLOB API base URL"
    )]
    clob_url: String,

    #[arg(
        long = "rpc-url",
        default_value = "https://polygon-rpc.com",
        help = "Polygon RPC URL"
    )]
    rpc_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClobApiResponse {
    #[serde(alias = "apiKey", alias = "api_key", alias = "key")]
    api_key: String,
    #[serde(alias = "secret", alias = "apiSecret", alias = "api_secret")]
    secret: String,
    #[serde(alias = "passphrase", alias = "apiPassphrase", alias = "api_passphrase")]
    passphrase: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    println!("============================================================");
    println!("🔐 TABULA TRADER: POLYMARKET CLOB API CREDENTIAL ONBOARDING");
    println!("============================================================");

    // 1. Resolve private key from CLI argument, environment, or interactive input
    let mut raw_key = args.private_key
        .or_else(|| std::env::var("PRIVATE_KEY").ok())
        .or_else(|| std::env::var("POLYMARKET_PRIVATE_KEY").ok())
        .unwrap_or_default();

    if raw_key.trim().is_empty() {
        print!("👉 Enter Polygon Wallet Private Key (64 hex characters): ");
        io::stdout().flush()?;
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer)?;
        raw_key = buffer;
    }

    let cleaned_key = raw_key.trim().trim_matches('"').trim_matches('\'').to_string();
    let formatted_key = if cleaned_key.starts_with("0x") {
        cleaned_key
    } else {
        format!("0x{}", cleaned_key)
    };

    // 2. Validate private key and derive wallet
    let wallet: LocalWallet = formatted_key.parse()
        .map_err(|e| format!("Invalid EVM private key: {}", e))?;
    let wallet = wallet.with_chain_id(137u64);
    let address = wallet.address();

    println!("   Wallet Address:   {:?}", address);
    println!("   Network:          Polygon Mainnet (Chain ID 137)");
    println!("   CLOB Endpoint:    {}", args.clob_url);
    println!("------------------------------------------------------------");

    let exchange_addr: Address = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E".parse()?;
    let signer = PolymarketSigner::new(&formatted_key, exchange_addr)?;
    let http_client = reqwest::Client::builder()
        .user_agent("py-clob-client/0.1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs()
        .to_string();

    println!("📡 Contacting Polymarket CLOB to derive / create Level-2 API keys...");

    // 3. Generate standard EIP-712 TypedData signature
    let signature = signer.sign_clob_auth_typed_data(&now_unix, 0).await?;

    let mut headers = HeaderMap::new();
    headers.insert("POLY_ADDRESS", format!("{:?}", address).parse()?);
    headers.insert("POLY_SIGNATURE", signature.parse()?);
    headers.insert("POLY_TIMESTAMP", now_unix.parse()?);
    headers.insert("POLY_NONCE", "0".parse()?);

    // 4. Register or create fresh Level-2 API credentials
    println!("⚡ Registering/Generating full Level-2 API Key with Polymarket CLOB...");
    let mut create_headers = headers.clone();
    create_headers.insert("Content-Type", "application/json".parse()?);

    let create_url = format!("{}/auth/api-key", args.clob_url.trim_end_matches('/'));
    let create_resp = http_client
        .post(&create_url)
        .headers(create_headers.clone())
        .body("{}")
        .send()
        .await;

    let mut credentials: Option<ClobApiResponse> = None;

    if let Ok(r) = create_resp {
        if r.status().is_success() {
            if let Ok(creds) = r.json::<ClobApiResponse>().await {
                println!("   ✅ Fresh Level-2 Polymarket API key registered on CLOB!");
                credentials = Some(creds);
            }
        } else {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            println!("   ℹ️ Create response (HTTP {}): {}. Attempting derive...", status, body);
        }
    }

    if credentials.is_none() {
        let derive_url = format!("{}/auth/derive-api-key", args.clob_url.trim_end_matches('/'));
        let derive_resp = http_client
            .get(&derive_url)
            .headers(headers.clone())
            .send()
            .await?;

        if !derive_resp.status().is_success() {
            let status = derive_resp.status();
            let body = derive_resp.text().await.unwrap_or_default();
            return Err(format!(
                "Polymarket CLOB API Key derivation failed (HTTP {}): {}",
                status, body
            ).into());
        }

        let creds: ClobApiResponse = derive_resp.json().await?;
        println!("   ✅ Existing Polymarket API credentials derived!");
        credentials = Some(creds);
    }

    let creds = credentials.expect("Credentials must be populated");

    // 5. Write to .secrets/trading.env securely
    let env_path = Path::new(&args.env_file);
    if let Some(parent) = env_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let env_content = format!(
        "# ============================================================\n\
         # TABULA TRADER: LIVE TRADING CREDENTIALS\n\
         # Auto-generated by generate_polymarket_credentials\n\
         # ============================================================\n\
         POLYMARKET_PRIVATE_KEY=\"{}\"\n\
         POLYMARKET_WALLET_ADDRESS=\"{:?}\"\n\
         POLY_API_KEY=\"{}\"\n\
         POLY_SECRET=\"{}\"\n\
         POLY_PASSPHRASE=\"{}\"\n\
         POLYGON_RPC_URL=\"{}\"\n",
        formatted_key,
        address,
        creds.api_key,
        creds.secret,
        creds.passphrase,
        args.rpc_url,
    );

    fs::write(env_path, env_content)?;

    // On Unix, restrict permissions to read/write by owner only (0600)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(env_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(env_path, perms)?;
    }

    println!("------------------------------------------------------------");
    println!("🎉 CREDENTIAL ONBOARDING COMPLETE!");
    println!("   📁 Credentials saved to: {}", args.env_file);
    println!("   🔑 POLY_API_KEY:         {}", creds.api_key);
    println!("   🛡️ POLY_PASSPHRASE:      {}", creds.passphrase);
    println!("   🌐 POLYGON_RPC_URL:      {}", args.rpc_url);
    println!("============================================================");
    println!("💡 NEXT STEPS:");
    println!("   1. Send USDC.e (Polygon) and POL (MATIC for gas) to:");
    println!("      👉 {:?}", address);
    println!("   2. You can now launch live trading anytime with:");
    println!("      $ cargo run --release --bin trading_node");
    println!("============================================================");

    Ok(())
}
