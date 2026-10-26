//! Compare against OpenZeppelin's reference JavaScript implementation (@openzeppelin/merkle-tree).
//! The vectors in fixtures/oz_vectors.json were produced by fixtures/generate.mjs.

use merkle_airdrop::distribution::{hash_from_hex, hash_to_hex};
use merkle_airdrop::{leaf_hash, verify, Address, MerkleTree, U256};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct VectorClaim {
    address: String,
    amount: String,
    proof: Vec<String>,
}

#[derive(Deserialize)]
struct VectorSet {
    root: String,
    claims: Vec<VectorClaim>,
}

fn vectors() -> BTreeMap<String, VectorSet> {
    let raw = include_str!("fixtures/oz_vectors.json");
    serde_json::from_str(raw).expect("fixture parses")
}

fn build(set: &VectorSet) -> MerkleTree {
    let claims = set
        .claims
        .iter()
        .map(|c| {
            (
                Address::parse(&c.address).unwrap(),
                U256::from_decimal(&c.amount).unwrap(),
            )
        })
        .collect();
    MerkleTree::new(claims).unwrap()
}

#[test]
fn fixture_covers_the_interesting_shapes() {
    let v = vectors();
    for name in [
        "single",
        "two",
        "three",
        "five_big_amounts",
        "eight",
        "eleven",
    ] {
        assert!(v.contains_key(name), "missing {name}");
    }
}

#[test]
fn roots_match_openzeppelin() {
    for (name, set) in vectors() {
        let tree = build(&set);
        assert_eq!(
            hash_to_hex(&tree.root()),
            set.root,
            "root mismatch for set `{name}`"
        );
    }
}

#[test]
fn proofs_match_openzeppelin_exactly() {
    for (name, set) in vectors() {
        let tree = build(&set);
        for claim in &set.claims {
            let address = Address::parse(&claim.address).unwrap();
            let ours: Vec<String> = tree
                .proof_for(&address)
                .unwrap()
                .iter()
                .map(hash_to_hex)
                .collect();
            assert_eq!(
                ours, claim.proof,
                "proof mismatch for {} in set `{name}`",
                claim.address
            );
        }
    }
}

#[test]
fn openzeppelin_proofs_verify_with_our_verifier() {
    for (name, set) in vectors() {
        let root = hash_from_hex(&set.root).unwrap();
        for claim in &set.claims {
            let leaf = leaf_hash(
                &Address::parse(&claim.address).unwrap(),
                &U256::from_decimal(&claim.amount).unwrap(),
            );
            let proof: Vec<_> = claim
                .proof
                .iter()
                .map(|p| hash_from_hex(p).unwrap())
                .collect();
            assert!(
                verify(&root, &leaf, &proof),
                "{} in `{name}` did not verify",
                claim.address
            );
        }
    }
}
