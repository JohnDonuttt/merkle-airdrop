//! Build and verify Merkle trees for token airdrops.
//!
//! Roots and proofs match OpenZeppelin's `StandardMerkleTree` (for `["address", "uint256"]`
//! leaves) and verify with `MerkleProof.verify` in Solidity.

pub mod address;
pub mod distribution;
pub mod error;
pub mod tree;
pub mod u256;

pub use address::Address;
pub use distribution::{Claim, Distribution};
pub use error::{Error, Result};
pub use tree::{leaf_hash, verify, MerkleTree};
pub use u256::U256;
