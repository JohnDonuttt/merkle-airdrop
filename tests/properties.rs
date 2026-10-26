use merkle_airdrop::tree::{hash_pair, keccak};
use merkle_airdrop::{leaf_hash, verify, Address, MerkleTree, U256};
use proptest::collection::hash_map;
use proptest::prelude::*;

fn claim_set() -> impl Strategy<Value = Vec<(Address, U256)>> {
    // a hash map guarantees unique addresses, which the tree requires
    hash_map(any::<[u8; 20]>(), any::<[u8; 32]>(), 1..60).prop_map(|m| {
        m.into_iter()
            .map(|(a, v)| {
                let mut amount = v;
                if amount.iter().all(|b| *b == 0) {
                    amount[31] = 1; // zero amounts are rejected upstream
                }
                (Address(a), U256(amount))
            })
            .collect()
    })
}

proptest! {
    #[test]
    fn every_proof_verifies(claims in claim_set()) {
        let tree = MerkleTree::new(claims).unwrap();
        for e in tree.entries() {
            let proof = tree.proof_for(&e.address).unwrap();
            prop_assert!(verify(&tree.root(), &e.leaf, &proof));
        }
    }

    #[test]
    fn root_is_independent_of_input_order(claims in claim_set(), seed in any::<u64>()) {
        let mut shuffled = claims.clone();
        // deterministic shuffle driven by the seed
        let n = shuffled.len();
        let mut s = seed;
        for i in (1..n).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            shuffled.swap(i, (s >> 33) as usize % (i + 1));
        }
        prop_assert_eq!(MerkleTree::new(claims).unwrap().root(), MerkleTree::new(shuffled).unwrap().root());
    }

    #[test]
    fn a_different_amount_never_verifies(claims in claim_set(), pick in any::<usize>(), delta in 1u8..=255) {
        let tree = MerkleTree::new(claims).unwrap();
        let e = &tree.entries()[pick % tree.len()];
        let proof = tree.proof_for(&e.address).unwrap();

        let mut tampered = e.amount;
        tampered.0[31] ^= delta; // always changes the value
        prop_assert!(!verify(&tree.root(), &leaf_hash(&e.address, &tampered), &proof));
    }

    #[test]
    fn a_proof_does_not_work_for_someone_elses_leaf(claims in claim_set(), a in any::<usize>(), b in any::<usize>()) {
        let tree = MerkleTree::new(claims).unwrap();
        prop_assume!(tree.len() > 1);
        let (i, j) = (a % tree.len(), b % tree.len());
        prop_assume!(i != j);

        let proof_i = tree.proof_for(&tree.entries()[i].address).unwrap();
        let leaf_j = tree.entries()[j].leaf;
        // proofs of different leaves are different paths; swapping them must fail
        // (the one exception is the two-leaf tree, where both proofs are the other's leaf)
        if proof_i != tree.proof_for(&tree.entries()[j].address).unwrap() {
            prop_assert!(!verify(&tree.root(), &leaf_j, &proof_i));
        }
    }

    #[test]
    fn proof_length_is_at_most_ceil_log2(claims in claim_set()) {
        let tree = MerkleTree::new(claims).unwrap();
        let bound = (usize::BITS - (tree.len() - 1).leading_zeros()) as usize; // ceil(log2(n)), 0 for n = 1
        for e in tree.entries() {
            prop_assert!(tree.proof_for(&e.address).unwrap().len() <= bound);
        }
    }

    #[test]
    fn hash_pair_is_commutative(a in any::<[u8; 32]>(), b in any::<[u8; 32]>()) {
        prop_assert_eq!(hash_pair(&a, &b), hash_pair(&b, &a));
    }

    #[test]
    fn u256_decimal_round_trips(bytes in any::<[u8; 32]>()) {
        let v = U256(bytes);
        prop_assert_eq!(U256::from_decimal(&v.to_string()).unwrap(), v);
    }

    #[test]
    fn leaf_never_equals_a_pair_hash_of_its_own_halves(a in any::<[u8; 20]>(), v in any::<[u8; 32]>()) {
        // the reason leaves are hashed twice: a 64-byte leaf preimage must not double as an internal node
        let leaf = leaf_hash(&Address(a), &U256(v));
        let mut encoded = [0u8; 64];
        encoded[12..32].copy_from_slice(&a);
        encoded[32..].copy_from_slice(&v);
        let (l, r) = (&encoded[..32], &encoded[32..]);
        let as_node = hash_pair(&<[u8; 32]>::try_from(l).unwrap(), &<[u8; 32]>::try_from(r).unwrap());
        prop_assert_ne!(leaf, as_node);
        prop_assert_ne!(leaf, keccak(&encoded));
    }
}
