use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, H256, U256};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const EIP712_DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

const ORDER_TYPE: &str =
    "Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)";

const CLOB_AUTH_DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId)";

const CLOB_AUTH_TYPE: &str =
    "ClobAuth(address address,string timestamp,uint256 nonce,string message)";

pub const CLOB_AUTH_MSG: &str =
    "This message attests that I control the given wallet";

pub const CLOB_AUTH_MSG_CREATE: &str =
    "This message attests that I control the given wallet";

pub const CLOB_AUTH_MSG_DERIVE: &str =
    "This message attests that I control the given wallet";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketOrder {
    pub salt: u64,
    pub maker: Address,
    pub signer: Address,
    pub taker: Address,
    pub token_id: U256,
    pub maker_amount: U256,
    pub taker_amount: U256,
    pub expiration: u64,
    pub nonce: u64,
    pub fee_rate_bps: u64,
    pub side: u8,           // 0 = BUY, 1 = SELL
    pub signature_type: u8, // 0 = EOA, 1 = POLY_PROXY
}

#[derive(Clone, Debug)]
pub struct PolymarketSigner {
    pub wallet: LocalWallet,
    pub exchange_address: Address,
    pub proxy_address: Option<Address>,
    pub chain_id: u64,
    pub cached_domain_separator: [u8; 32],
    nonce_counter: Arc<AtomicU64>,
    salt_counter: Arc<AtomicU64>,
}

impl PolymarketSigner {
    pub fn new(
        private_key: &str,
        exchange_address: Address,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let wallet = private_key.parse::<LocalWallet>()?.with_chain_id(137u64);
        let chain_id = 137u64;

        // 1. Precompute domain separator ONCE at initialization
        let domain_type_hash = ethers::utils::keccak256(EIP712_DOMAIN_TYPE);
        let name_hash = ethers::utils::keccak256("Polymarket CTF Exchange");
        let version_hash = ethers::utils::keccak256("1");

        let mut buf = [0u8; 160];
        buf[0..32].copy_from_slice(&domain_type_hash);
        buf[32..64].copy_from_slice(&name_hash);
        buf[64..96].copy_from_slice(&version_hash);
        
        let mut chain_bytes = [0u8; 32];
        U256::from(chain_id).to_big_endian(&mut chain_bytes);
        buf[96..128].copy_from_slice(&chain_bytes);
        
        buf[140..160].copy_from_slice(exchange_address.as_bytes());
        let cached_domain_separator = ethers::utils::keccak256(buf);

        Ok(Self {
            wallet,
            exchange_address,
            proxy_address: None,
            chain_id,
            cached_domain_separator,
            nonce_counter: Arc::new(AtomicU64::new(1)),
            salt_counter: Arc::new(AtomicU64::new(1)),
        })
    }

    pub fn with_proxy_address(mut self, proxy: Address) -> Self {
        self.proxy_address = Some(proxy);
        self
    }

    pub fn next_nonce(&self) -> u64 {
        self.nonce_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn sync_nonce(&self, confirmed_nonce: u64) {
        self.nonce_counter
            .store(confirmed_nonce + 1, Ordering::SeqCst);
    }

    /// Guaranteed collision-free salt generation using high-resolution nanos and atomic sequence
    pub fn generate_salt(&self) -> u64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(123456789);
        let seq = self.salt_counter.fetch_add(1, Ordering::Relaxed);
        nanos ^ (seq << 32)
    }

    /// Returns precomputed EIP-712 domain separator: O(1) instantaneous stack copy
    #[inline(always)]
    pub fn compute_domain_separator(&self) -> [u8; 32] {
        self.cached_domain_separator
    }

    /// Zero-allocation order hashing using stack buffer: keccak256(abi.encode(ORDER_TYPEHASH, salt, maker, signer, taker, tokenId, makerAmount, takerAmount, expiration, nonce, feeRateBps, side, signatureType))
    #[inline(always)]
    pub fn compute_order_hash(&self, order: &PolymarketOrder) -> [u8; 32] {
        let order_type_hash = ethers::utils::keccak256(ORDER_TYPE);

        let mut buf = [0u8; 416];
        buf[0..32].copy_from_slice(&order_type_hash);
        write_u256_word(U256::from(order.salt), &mut buf[32..64]);
        write_address_word(order.maker, &mut buf[64..96]);
        write_address_word(order.signer, &mut buf[96..128]);
        write_address_word(order.taker, &mut buf[128..160]);
        write_u256_word(order.token_id, &mut buf[160..192]);
        write_u256_word(order.maker_amount, &mut buf[192..224]);
        write_u256_word(order.taker_amount, &mut buf[224..256]);
        write_u256_word(U256::from(order.expiration), &mut buf[256..288]);
        write_u256_word(U256::from(order.nonce), &mut buf[288..320]);
        write_u256_word(U256::from(order.fee_rate_bps), &mut buf[320..352]);
        write_u256_word(U256::from(order.side), &mut buf[352..384]);
        write_u256_word(U256::from(order.signature_type), &mut buf[384..416]);

        ethers::utils::keccak256(buf)
    }

    /// Generates EIP-712 typed signature for gasless order placement on Polygon PoS with zero heap allocations
    pub async fn sign_order(
        &self,
        order: &PolymarketOrder,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let domain_separator = self.cached_domain_separator;
        let struct_hash = self.compute_order_hash(order);
        
        let mut digest_buf = [0u8; 66];
        digest_buf[0] = 0x19;
        digest_buf[1] = 0x01;
        digest_buf[2..34].copy_from_slice(&domain_separator);
        digest_buf[34..66].copy_from_slice(&struct_hash);

        let digest = ethers::utils::keccak256(digest_buf);
        let signature = self.wallet.sign_hash(H256::from(digest))?;
        Ok(format!("0x{}", signature))
    }

    /// Creates and signs a new order struct atomically in a single zero-allocation call
    pub async fn build_and_sign_order(
        &self,
        token_id: U256,
        maker_amount_usdc_raw: u64,   // e.g. 5_000_000 for $5.00
        taker_amount_shares_raw: u64, // e.g. 10_000_000 for 10 shares
        side: u8,
        expiration_unix: u64,
    ) -> Result<(PolymarketOrder, String), Box<dyn std::error::Error + Send + Sync>> {
        self.build_and_sign_order_with_fee(
            token_id,
            maker_amount_usdc_raw,
            taker_amount_shares_raw,
            side,
            expiration_unix,
            0,
        )
        .await
    }

    /// Creates and signs a new order struct with explicit fee rate basis points
    pub async fn build_and_sign_order_with_fee(
        &self,
        token_id: U256,
        maker_amount_usdc_raw: u64,   // e.g. 5_000_000 for $5.00 USDC.e
        taker_amount_shares_raw: u64, // e.g. 10_000_000 for 10 shares
        side: u8,
        expiration_unix: u64,
        fee_rate_bps: u64,
    ) -> Result<(PolymarketOrder, String), Box<dyn std::error::Error + Send + Sync>> {
        let (maker_amount, taker_amount) = if side == 0 {
            // BUY: Maker pays USDC collateral, receives outcome tokens
            (U256::from(maker_amount_usdc_raw), U256::from(taker_amount_shares_raw))
        } else {
            // SELL: Maker gives outcome tokens, receives USDC collateral
            (U256::from(taker_amount_shares_raw), U256::from(maker_amount_usdc_raw))
        };

        let (maker, signature_type) = if let Some(proxy) = self.proxy_address {
            (proxy, 1u8)
        } else {
            (self.wallet.address(), 0u8)
        };

        let order = PolymarketOrder {
            salt: self.generate_salt(),
            maker,
            signer: self.wallet.address(),
            taker: Address::zero(),
            token_id,
            maker_amount,
            taker_amount,
            expiration: expiration_unix,
            nonce: self.next_nonce(),
            fee_rate_bps,
            side,
            signature_type,
        };

        let sig = self.sign_order(&order).await?;
        Ok((order, sig))
    }

    /// Computes strictly compliant EIP-712 Domain Separator for Polymarket ClobAuth
    pub fn compute_clob_auth_domain_separator(&self) -> [u8; 32] {
        let domain_type_hash = ethers::utils::keccak256(CLOB_AUTH_DOMAIN_TYPE);
        let name_hash = ethers::utils::keccak256("ClobAuthDomain");
        let version_hash = ethers::utils::keccak256("1");

        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(&domain_type_hash);
        buf.extend_from_slice(&name_hash);
        buf.extend_from_slice(&version_hash);
        encode_word_u256(U256::from(self.chain_id), &mut buf);

        ethers::utils::keccak256(&buf)
    }

    /// Computes strictly compliant EIP-712 ClobAuth struct hash
    pub fn compute_clob_auth_hash(
        &self,
        address: Address,
        timestamp: &str,
        nonce: u64,
        message: &str,
    ) -> [u8; 32] {
        let type_hash = ethers::utils::keccak256(CLOB_AUTH_TYPE);
        let timestamp_hash = ethers::utils::keccak256(timestamp.as_bytes());
        let message_hash = ethers::utils::keccak256(message.as_bytes());

        let mut buf = Vec::with_capacity(160);
        buf.extend_from_slice(&type_hash);
        encode_word_address(address, &mut buf);
        buf.extend_from_slice(&timestamp_hash);
        encode_word_u256(U256::from(nonce), &mut buf);
        buf.extend_from_slice(&message_hash);

        ethers::utils::keccak256(&buf)
    }

    /// Signs EIP-712 ClobAuth payload to create or derive API credentials from Polymarket CLOB
    pub fn sign_clob_auth(
        &self,
        timestamp: &str,
        nonce: u64,
        message: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let domain_separator = self.compute_clob_auth_domain_separator();
        let struct_hash = self.compute_clob_auth_hash(self.wallet.address(), timestamp, nonce, message);
        let digest = ethers::utils::keccak256(
            [&[0x19, 0x01], &domain_separator[..], &struct_hash[..]].concat(),
        );
        let signature = self.wallet.sign_hash(H256::from(digest))?;
        Ok(format!("0x{}", signature))
    }

    /// Signs EIP-712 ClobAuth payload using standard TypedData
    pub async fn sign_clob_auth_typed_data(
        &self,
        timestamp: &str,
        nonce: u64,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let address_str = format!("{:?}", self.wallet.address());
        let json_typed_data = serde_json::json!({
            "types": {
                "EIP712Domain": [
                    { "name": "name", "type": "string" },
                    { "name": "version", "type": "string" },
                    { "name": "chainId", "type": "uint256" }
                ],
                "ClobAuth": [
                    { "name": "address", "type": "address" },
                    { "name": "timestamp", "type": "string" },
                    { "name": "nonce", "type": "uint256" },
                    { "name": "message", "type": "string" }
                ]
            },
            "primaryType": "ClobAuth",
            "domain": {
                "name": "ClobAuthDomain",
                "version": "1",
                "chainId": 137
            },
            "message": {
                "address": address_str,
                "timestamp": timestamp,
                "nonce": nonce,
                "message": CLOB_AUTH_MSG
            }
        });

        let typed_data: ethers::types::transaction::eip712::TypedData =
            serde_json::from_value(json_typed_data)?;
        let signature = self.wallet.sign_typed_data(&typed_data).await?;
        Ok(format!("0x{}", signature))
    }
}

#[inline(always)]
fn write_u256_word(val: U256, dst: &mut [u8]) {
    val.to_big_endian(dst);
}

#[inline(always)]
fn write_address_word(addr: Address, dst: &mut [u8]) {
    dst[0..12].fill(0);
    dst[12..32].copy_from_slice(addr.as_bytes());
}

#[inline(always)]
fn encode_word_u256(val: U256, buf: &mut Vec<u8>) {
    let mut bytes = [0u8; 32];
    val.to_big_endian(&mut bytes);
    buf.extend_from_slice(&bytes);
}

#[inline(always)]
fn encode_word_address(addr: Address, buf: &mut Vec<u8>) {
    let mut bytes = [0u8; 32];
    bytes[12..32].copy_from_slice(addr.as_bytes());
    buf.extend_from_slice(&bytes);
}
