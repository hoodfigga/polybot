pub mod ctf_redeemer;
pub mod rest_client;
pub mod signer;
pub mod ws_client;

pub use ctf_redeemer::{CtfRedeemer, CtfRedemptionParams};
pub use rest_client::{PolymarketApiCredentials, PolymarketRestClient};
pub use signer::{PolymarketOrder, PolymarketSigner};
pub use ws_client::{MarketDeltaEvent, PolymarketWsClient};

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::signers::Signer;
    use ethers::types::{Address, U256};

    #[tokio::test]
    async fn test_eip712_signer_generation() {
        // Deterministic Alice private key for testing
        let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let exchange: Address = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"
            .parse()
            .unwrap();
        let signer = PolymarketSigner::new(pk, exchange).expect("Signer creation must succeed");

        let order = PolymarketOrder {
            salt: 123456789,
            maker: signer.wallet.address(),
            signer: signer.wallet.address(),
            taker: Address::zero(),
            token_id: U256::from(1001),
            maker_amount: U256::from(5000000), // $5.00 USDC.e (6 decimals)
            taker_amount: U256::from(10000000), // 10 shares
            expiration: 1780000000,
            nonce: 1,
            fee_rate_bps: 0,
            side: 0,           // BUY
            signature_type: 0, // EOA
        };

        let sig = signer
            .sign_order(&order)
            .await
            .expect("Signing must succeed");
        assert!(sig.starts_with("0x"));
        assert_eq!(sig.len(), 132); // 65 bytes in hex + 0x prefix
    }

    #[tokio::test]
    async fn test_salt_and_nonce_monotonicity() {
        let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let exchange: Address = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"
            .parse()
            .unwrap();
        let signer = PolymarketSigner::new(pk, exchange).unwrap();

        let n1 = signer.next_nonce();
        let n2 = signer.next_nonce();
        assert_eq!(n2, n1 + 1);

        let s1 = signer.generate_salt();
        let s2 = signer.generate_salt();
        assert_ne!(s1, s2);

        let (order, sig) = signer
            .build_and_sign_order(U256::from(555), 10_000_000, 20_000_000, 0, 1800000000)
            .await
            .unwrap();

        assert_eq!(order.token_id, U256::from(555));
        assert!(sig.starts_with("0x"));
    }

    #[test]
    fn test_dynamic_fee_preflight_threshold() {
        // Gross profit 2.5% (250 bps), fee rate 15 bps, hurdle 50 bps -> Profitable (250 >= 65)
        assert!(PolymarketRestClient::is_arbitrage_profitable_after_fees(
            2.5, 15, 50
        ));

        // Gross profit 1.0% (100 bps), dynamic high crypto taker fee 150 bps, hurdle 50 bps -> Rejected (100 < 200)
        assert!(!PolymarketRestClient::is_arbitrage_profitable_after_fees(
            1.0, 150, 50
        ));
    }

    #[test]
    fn test_l2_hmac_header_construction() {
        let creds = PolymarketApiCredentials {
            key: "test_api_key_uuid".to_string(),
            secret_base64: "c2VjcmV0X2J5dGVzX2Zvcl9obWFj".to_string(), // base64 "secret_bytes_for_hmac"
            passphrase: "test_passphrase".to_string(),
            address: "0x6674C3dC820B3A9dED849d02C8D7437783EA3Ead".to_string(),
        };

        let client = PolymarketRestClient::new("test_api_key_uuid").with_credentials(creds);
        let headers = client
            .build_l2_headers("POST", "/order", r#"{"order":"data"}"#)
            .expect("L2 headers must build successfully");

        assert_eq!(headers.get("POLY_API_KEY").unwrap(), "test_api_key_uuid");
        assert_eq!(headers.get("POLY_PASSPHRASE").unwrap(), "test_passphrase");
        assert!(headers.contains_key("POLY_SIGNATURE"));
        assert!(headers.contains_key("POLY_TIMESTAMP"));
    }

    #[test]
    fn test_clob_auth_signing() {
        let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let exchange: Address = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"
            .parse()
            .unwrap();
        let signer = PolymarketSigner::new(pk, exchange).unwrap();

        let sig = signer
            .sign_clob_auth(
                "1725000000",
                0,
                signer::CLOB_AUTH_MSG_CREATE,
            )
            .expect("ClobAuth signing must succeed");

        assert!(sig.starts_with("0x"));
        assert_eq!(sig.len(), 132); // 65 bytes in hex + 0x prefix
    }
}
