use crate::address::Address;
use crate::error::{Error, Result};
use crate::tree::{leaf_hash, verify, Hash, MerkleTree};
use crate::u256::U256;
use serde::{Deserialize, Serialize};
use std::io::Read;

/// What gets published: the root for the contract, and one entry per recipient with the proof
/// they submit when claiming.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Distribution {
    pub merkle_root: String,
    pub token_total: String,
    pub claims: Vec<Claim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claim {
    pub address: String,
    pub amount: String,
    pub proof: Vec<String>,
}

pub fn hash_to_hex(h: &Hash) -> String {
    format!("0x{}", hex::encode(h))
}

pub fn hash_from_hex(s: &str) -> Result<Hash> {
    let body = s
        .strip_prefix("0x")
        .ok_or_else(|| Error::BadHash(s.to_string()))?;
    let mut out = [0u8; 32];
    hex::decode_to_slice(body, &mut out).map_err(|_| Error::BadHash(s.to_string()))?;
    Ok(out)
}

/// Read `address,amount` rows. Whitespace is trimmed; row numbers in errors count the header as row 1.
pub fn parse_csv<R: Read>(reader: R) -> Result<Vec<(Address, U256)>> {
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(reader);

    let headers = rdr.headers()?.clone();
    let column = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (addr_col, amount_col) = match (column("address"), column("amount")) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(Error::MissingColumns),
    };

    let mut claims = Vec::new();
    for (i, record) in rdr.records().enumerate() {
        let record = record?;
        let row = i + 2;

        let raw_addr = record.get(addr_col).unwrap_or("");
        let address = Address::parse(raw_addr).map_err(|source| Error::Address {
            row,
            value: raw_addr.to_string(),
            source,
        })?;

        let raw_amount = record.get(amount_col).unwrap_or("");
        let amount = U256::from_decimal(raw_amount).map_err(|source| Error::Amount {
            row,
            value: raw_amount.to_string(),
            source,
        })?;
        if amount.is_zero() {
            return Err(Error::ZeroAmount { row });
        }

        claims.push((address, amount));
    }
    Ok(claims)
}

fn total(claims: &[(Address, U256)]) -> Result<U256> {
    claims
        .iter()
        .try_fold(U256::ZERO, |sum, (_, amount)| sum.checked_add(*amount))
        .ok_or(Error::TotalOverflow)
}

impl Distribution {
    pub fn build(claims: Vec<(Address, U256)>) -> Result<Self> {
        let token_total = total(&claims)?;
        let tree = MerkleTree::new(claims)?;

        let mut out: Vec<Claim> = tree
            .entries()
            .iter()
            .map(|e| Claim {
                address: e.address.to_string(),
                amount: e.amount.to_string(),
                proof: tree
                    .proof_for(&e.address)
                    .expect("entry is in the tree")
                    .iter()
                    .map(hash_to_hex)
                    .collect(),
            })
            .collect();
        // sorted by address so the file is easy to read and diff
        out.sort_by_key(|c| c.address.to_lowercase());

        Ok(Distribution {
            merkle_root: hash_to_hex(&tree.root()),
            token_total: token_total.to_string(),
            claims: out,
        })
    }

    pub fn find(&self, address: &Address) -> Option<&Claim> {
        self.claims.iter().find(|c| {
            Address::parse(&c.address)
                .map(|a| a == *address)
                .unwrap_or(false)
        })
    }

    /// Audit a distribution file: every proof must verify against the root, the root must match
    /// a fresh rebuild from the claims, and the total must add up. Returns the number of claims.
    pub fn check(&self) -> Result<usize> {
        let root = hash_from_hex(&self.merkle_root)?;

        let mut parsed = Vec::with_capacity(self.claims.len());
        for claim in &self.claims {
            let address = Address::parse(&claim.address).map_err(|source| Error::Address {
                row: 0,
                value: claim.address.clone(),
                source,
            })?;
            let amount = U256::from_decimal(&claim.amount).map_err(|source| Error::Amount {
                row: 0,
                value: claim.amount.clone(),
                source,
            })?;

            let proof: Vec<Hash> = claim
                .proof
                .iter()
                .map(|p| hash_from_hex(p))
                .collect::<Result<_>>()?;
            if !verify(&root, &leaf_hash(&address, &amount), &proof) {
                return Err(Error::BadProof {
                    address: claim.address.clone(),
                });
            }
            parsed.push((address, amount));
        }

        let computed_total = total(&parsed)?.to_string();
        if computed_total != self.token_total {
            return Err(Error::TotalMismatch {
                file: self.token_total.clone(),
                computed: computed_total,
            });
        }

        let rebuilt = hash_to_hex(&MerkleTree::new(parsed)?.root());
        if rebuilt != self.merkle_root.to_lowercase() {
            return Err(Error::RootMismatch {
                file: self.merkle_root.clone(),
                rebuilt,
            });
        }
        Ok(self.claims.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
    const B: &str = "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359";
    const C: &str = "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB";

    fn csv(rows: &str) -> Vec<(Address, U256)> {
        parse_csv(rows.as_bytes()).unwrap()
    }

    fn csv_err(rows: &str) -> String {
        parse_csv(rows.as_bytes()).unwrap_err().to_string()
    }

    fn sample() -> Distribution {
        Distribution::build(csv(&format!(
            "address,amount\n{A},100\n{B},250\n{C},1000000000000000000\n"
        )))
        .unwrap()
    }

    #[test]
    fn parses_rows_with_flexible_headers_and_whitespace() {
        let claims = csv(&format!("Amount , ADDRESS\n 5 , {A} \n"));
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].0.to_string(), A);
        assert_eq!(claims[0].1.to_string(), "5");
    }

    #[test]
    fn reports_the_row_number_of_bad_input() {
        let msg = csv_err(&format!("address,amount\n{A},1\nnot-an-address,2\n"));
        assert!(msg.starts_with("row 3: invalid address"), "{msg}");

        let msg = csv_err(&format!("address,amount\n{A},1.5\n"));
        assert!(msg.starts_with("row 2: invalid amount"), "{msg}");
    }

    #[test]
    fn rejects_zero_amounts_missing_columns_and_checksum_typos() {
        assert!(csv_err(&format!("address,amount\n{A},0\n")).contains("amount is zero"));
        assert!(csv_err("wallet,tokens\n1,2\n").contains("header row"));
        // a mixed-case address with one wrong letter-case must not slip through
        let typo = A.replacen('A', "a", 1);
        assert!(csv_err(&format!("address,amount\n{typo},1\n")).contains("checksum"));
    }

    #[test]
    fn totals_are_summed_exactly() {
        assert_eq!(sample().token_total, "1000000000000000350");
    }

    #[test]
    fn total_overflow_is_an_error() {
        let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
        let claims = csv(&format!("address,amount\n{A},{max}\n{B},1\n"));
        assert!(matches!(
            Distribution::build(claims),
            Err(Error::TotalOverflow)
        ));
    }

    #[test]
    fn built_distribution_passes_its_own_check() {
        assert_eq!(sample().check().unwrap(), 3);
    }

    #[test]
    fn claims_are_sorted_by_address_and_checksummed() {
        let d = sample();
        let addrs: Vec<_> = d.claims.iter().map(|c| c.address.as_str()).collect();
        assert_eq!(addrs, vec![A, C, B]);
    }

    #[test]
    fn find_is_case_insensitive() {
        let d = sample();
        let addr = Address::parse(&B.to_lowercase()).unwrap();
        assert_eq!(d.find(&addr).unwrap().amount, "250");
    }

    #[test]
    fn check_catches_a_tampered_amount() {
        let mut d = sample();
        d.claims[0].amount = "101".into();
        assert!(matches!(d.check(), Err(Error::BadProof { .. })));
    }

    #[test]
    fn check_catches_a_tampered_proof() {
        let mut d = sample();
        let flipped = hash_to_hex(&crate::tree::keccak(b"evil"));
        d.claims[1].proof[0] = flipped;
        assert!(matches!(d.check(), Err(Error::BadProof { .. })));
    }

    #[test]
    fn check_catches_a_wrong_total_and_a_wrong_root() {
        let mut d = sample();
        d.token_total = "1".into();
        assert!(matches!(d.check(), Err(Error::TotalMismatch { .. })));

        let mut d = sample();
        d.merkle_root = hash_to_hex(&crate::tree::keccak(b"other root"));
        assert!(matches!(d.check(), Err(Error::BadProof { .. })));
    }

    #[test]
    fn check_catches_a_claim_added_without_updating_the_root() {
        let mut d = sample();
        let extra =
            Distribution::build(csv(&format!("address,amount\n{A},100\n{B},250\n"))).unwrap();
        d.claims.push(extra.claims[0].clone());
        // the duplicate address trips the rebuild even though its proof verifies
        d.token_total = "1000000000000000450".into();
        assert!(d.check().is_err());
    }

    #[test]
    fn hash_hex_round_trips_and_rejects_garbage() {
        let h = crate::tree::keccak(b"x");
        assert_eq!(hash_from_hex(&hash_to_hex(&h)).unwrap(), h);
        assert!(hash_from_hex("0x1234").is_err());
        assert!(hash_from_hex("1234").is_err());
        assert!(hash_from_hex(&format!("0x{}", "zz".repeat(32))).is_err());
    }

    #[test]
    fn json_round_trip() {
        let d = sample();
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"merkleRoot\""));
        let back: Distribution = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }
}
