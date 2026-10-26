use crate::address::Address;
use crate::error::{Error, Result};
use crate::u256::U256;
use std::collections::HashSet;
use tiny_keccak::{Hasher, Keccak};

pub type Hash = [u8; 32];

pub fn keccak(data: &[u8]) -> Hash {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

/// Leaf = keccak256(keccak256(abi.encode(address, uint256))).
///
/// The second hash is what OpenZeppelin's StandardMerkleTree does so that a leaf (64 bytes of
/// data hashed once) can never be confused with an internal node (two 32-byte hashes hashed once).
pub fn leaf_hash(address: &Address, amount: &U256) -> Hash {
    let mut encoded = [0u8; 64];
    encoded[12..32].copy_from_slice(&address.0); // abi.encode left-pads an address to 32 bytes
    encoded[32..].copy_from_slice(&amount.0);
    keccak(&keccak(&encoded))
}

/// Hash two siblings in sorted order. Because order doesn't matter, a proof is just a list of
/// siblings with no left/right flags, which is what OpenZeppelin's MerkleProof.verify expects.
pub fn hash_pair(a: &Hash, b: &Hash) -> Hash {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(lo);
    buf[32..].copy_from_slice(hi);
    keccak(&buf)
}

/// Check a proof the same way the Solidity library does.
pub fn verify(root: &Hash, leaf: &Hash, proof: &[Hash]) -> bool {
    proof
        .iter()
        .fold(*leaf, |acc, sibling| hash_pair(&acc, sibling))
        == *root
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub address: Address,
    pub amount: U256,
    pub leaf: Hash,
}

/// A Merkle tree laid out like OpenZeppelin's StandardMerkleTree, so roots and proofs match
/// the reference JavaScript implementation byte for byte.
///
/// Leaves are sorted by hash, then stored in a flat array: the root is at index 0, the children
/// of node `i` are `2i+1` and `2i+2`, and leaves fill the end of the array in reverse. There is
/// no padding or duplicated leaves, so any number of claims works.
pub struct MerkleTree {
    nodes: Vec<Hash>,
    entries: Vec<Entry>, // sorted by leaf hash; entries[k] lives at nodes[len - 1 - k]
}

impl MerkleTree {
    pub fn new(claims: Vec<(Address, U256)>) -> Result<Self> {
        if claims.is_empty() {
            return Err(Error::Empty);
        }

        let mut seen = HashSet::with_capacity(claims.len());
        for (address, _) in &claims {
            if !seen.insert(*address) {
                return Err(Error::DuplicateAddress(address.to_string()));
            }
        }

        let mut entries: Vec<Entry> = claims
            .into_iter()
            .map(|(address, amount)| Entry {
                leaf: leaf_hash(&address, &amount),
                address,
                amount,
            })
            .collect();
        entries.sort_by_key(|e| e.leaf);

        let n = entries.len();
        let len = 2 * n - 1;
        let mut nodes = vec![[0u8; 32]; len];

        for (k, entry) in entries.iter().enumerate() {
            nodes[len - 1 - k] = entry.leaf;
        }
        for i in (0..len - n).rev() {
            nodes[i] = hash_pair(&nodes[2 * i + 1], &nodes[2 * i + 2]);
        }

        Ok(MerkleTree { nodes, entries })
    }

    pub fn root(&self) -> Hash {
        self.nodes[0]
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries in leaf order (sorted by hash), not input order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn proof_for(&self, address: &Address) -> Option<Vec<Hash>> {
        let k = self.entries.iter().position(|e| e.address == *address)?;
        Some(self.proof_at(k))
    }

    fn proof_at(&self, k: usize) -> Vec<Hash> {
        let mut index = self.nodes.len() - 1 - k;
        let mut proof = Vec::new();

        while index > 0 {
            let sibling = if index % 2 == 1 { index + 1 } else { index - 1 };
            proof.push(self.nodes[sibling]);
            index = (index - 1) / 2;
        }
        proof
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(n: u8) -> Address {
        let mut b = [0u8; 20];
        b[19] = n;
        Address(b)
    }

    fn amount(n: u64) -> U256 {
        U256::from_decimal(&n.to_string()).unwrap()
    }

    fn claims(n: u8) -> Vec<(Address, U256)> {
        (1..=n).map(|i| (addr(i), amount(i as u64 * 100))).collect()
    }

    #[test]
    fn keccak_matches_known_vector() {
        // keccak256("") is the well-known empty-input hash (and differs from NIST SHA3-256)
        assert_eq!(
            hex::encode(keccak(b"")),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    #[test]
    fn hash_pair_ignores_argument_order() {
        let a = keccak(b"a");
        let b = keccak(b"b");
        assert_eq!(hash_pair(&a, &b), hash_pair(&b, &a));
    }

    #[test]
    fn leaf_is_double_hashed() {
        let a = addr(1);
        let v = amount(5);
        let mut encoded = [0u8; 64];
        encoded[12..32].copy_from_slice(&a.0);
        encoded[32..].copy_from_slice(&v.0);
        assert_eq!(leaf_hash(&a, &v), keccak(&keccak(&encoded)));
        assert_ne!(leaf_hash(&a, &v), keccak(&encoded));
    }

    #[test]
    fn single_claim_root_is_the_leaf_with_an_empty_proof() {
        let tree = MerkleTree::new(claims(1)).unwrap();
        assert_eq!(tree.root(), leaf_hash(&addr(1), &amount(100)));
        assert_eq!(tree.proof_for(&addr(1)).unwrap(), Vec::<Hash>::new());
    }

    #[test]
    fn every_claim_verifies_for_many_sizes() {
        for n in 1..=40u8 {
            let tree = MerkleTree::new(claims(n)).unwrap();
            assert_eq!(tree.len(), n as usize);
            for e in tree.entries() {
                let proof = tree.proof_for(&e.address).unwrap();
                assert!(
                    verify(&tree.root(), &e.leaf, &proof),
                    "n={n} address={}",
                    e.address
                );
            }
        }
    }

    #[test]
    fn wrong_amount_or_wrong_address_fails() {
        let tree = MerkleTree::new(claims(7)).unwrap();
        let proof = tree.proof_for(&addr(3)).unwrap();

        assert!(verify(
            &tree.root(),
            &leaf_hash(&addr(3), &amount(300)),
            &proof
        ));
        assert!(!verify(
            &tree.root(),
            &leaf_hash(&addr(3), &amount(301)),
            &proof
        ));
        assert!(!verify(
            &tree.root(),
            &leaf_hash(&addr(4), &amount(300)),
            &proof
        ));
    }

    #[test]
    fn root_does_not_depend_on_input_order() {
        let forward = claims(9);
        let mut backward = claims(9);
        backward.reverse();
        assert_eq!(
            MerkleTree::new(forward).unwrap().root(),
            MerkleTree::new(backward).unwrap().root()
        );
    }

    #[test]
    fn proof_length_is_logarithmic() {
        let tree = MerkleTree::new(claims(100)).unwrap();
        let longest = tree
            .entries()
            .iter()
            .map(|e| tree.proof_for(&e.address).unwrap().len())
            .max()
            .unwrap();
        assert!(longest <= 7, "proof of {longest} hashes for 100 leaves");
    }

    #[test]
    fn rejects_empty_and_duplicate_input() {
        assert!(matches!(MerkleTree::new(vec![]), Err(Error::Empty)));

        let dup = vec![
            (addr(1), amount(1)),
            (addr(2), amount(2)),
            (addr(1), amount(3)),
        ];
        assert!(matches!(
            MerkleTree::new(dup),
            Err(Error::DuplicateAddress(_))
        ));
    }

    #[test]
    fn unknown_address_has_no_proof() {
        let tree = MerkleTree::new(claims(3)).unwrap();
        assert!(tree.proof_for(&addr(99)).is_none());
    }
}
