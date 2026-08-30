use ethers::abi::Token;
use ethers::types::{Address, Bytes, H256, U256};
use serde::{Deserialize, Serialize};

/// Polymarket Conditional Tokens (CTF) Contract on Polygon PoS (Plan2.md Module 5)
pub const POLYGON_CTF_EXCHANGE: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
pub const POLYGON_USDC_COLLATERAL: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"; // USDC.e on Polygon

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfRedemptionParams {
    pub collateral_token: Address,
    pub parent_collection_id: H256,
    pub condition_id: H256,
    pub index_sets: Vec<U256>,
}

pub struct CtfRedeemer {
    pub ctf_contract_address: Address,
    pub collateral_token_address: Address,
}

impl CtfRedeemer {
    pub fn new() -> Self {
        Self {
            ctf_contract_address: POLYGON_CTF_EXCHANGE.parse().unwrap(),
            collateral_token_address: POLYGON_USDC_COLLATERAL.parse().unwrap(),
        }
    }

    /// Encodes `redeemPositions(address,bytes32,bytes32,uint256[])` calldata
    pub fn encode_redeem_positions_calldata(
        &self,
        condition_id: H256,
        index_sets: &[u64],
    ) -> Bytes {
        // Selector for `redeemPositions(address,bytes32,bytes32,uint256[])`
        let selector = ethers::utils::id("redeemPositions(address,bytes32,bytes32,uint256[])");
        let mut calldata = Vec::with_capacity(164);
        calldata.extend_from_slice(&selector[0..4]);

        let index_set_tokens: Vec<Token> = index_sets
            .iter()
            .map(|&idx| Token::Uint(U256::from(idx)))
            .collect();

        let tokens = vec![
            Token::Address(self.collateral_token_address),
            Token::FixedBytes(H256::zero().as_bytes().to_vec()), // parentCollectionId = 0x0
            Token::FixedBytes(condition_id.as_bytes().to_vec()),
            Token::Array(index_set_tokens),
        ];

        let encoded = ethers::abi::encode(&tokens);
        calldata.extend_from_slice(&encoded);
        Bytes::from(calldata)
    }

    /// Evaluates if condition is eligible for redemption based on resolution outcome
    pub fn should_redeem_market(is_resolved: bool, winning_outcome_known: bool) -> bool {
        is_resolved && winning_outcome_known
    }
}

impl Default for CtfRedeemer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ctf_redemption_calldata_encoding() {
        let redeemer = CtfRedeemer::new();
        let condition_id: H256 = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
            .parse()
            .unwrap();

        let calldata = redeemer.encode_redeem_positions_calldata(condition_id, &[1, 2]);
        assert!(!calldata.is_empty());
        assert!(calldata.len() > 100);

        // Selector must be 4 bytes
        assert_eq!(&calldata[0..4], &ethers::utils::id("redeemPositions(address,bytes32,bytes32,uint256[])")[0..4]);
    }
}
