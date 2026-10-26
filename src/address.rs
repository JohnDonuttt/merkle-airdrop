use crate::error::ParseAddressError;
use crate::tree::keccak;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Address(pub [u8; 20]);

impl Address {
    /// Parse `0x` + 40 hex chars. Mixed-case input must carry a valid EIP-55 checksum, which
    /// catches typos; all-lowercase and all-uppercase are accepted as unchecked.
    pub fn parse(s: &str) -> Result<Self, ParseAddressError> {
        let hex_part = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .ok_or(ParseAddressError::MissingPrefix)?;

        if hex_part.len() != 40 {
            return Err(ParseAddressError::BadLength(hex_part.len()));
        }

        let mut bytes = [0u8; 20];
        hex::decode_to_slice(hex_part, &mut bytes).map_err(|_| ParseAddressError::NotHex)?;
        let addr = Address(bytes);

        let has_upper = hex_part.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = hex_part.chars().any(|c| c.is_ascii_lowercase());
        if has_upper && has_lower {
            let expected = addr.to_checksum();
            if expected[2..] != *hex_part {
                return Err(ParseAddressError::BadChecksum { expected });
            }
        }
        Ok(addr)
    }

    /// EIP-55: uppercase each hex letter whose matching nibble in keccak256(lowercase hex) is >= 8.
    pub fn to_checksum(&self) -> String {
        let lower = hex::encode(self.0);
        let hash = keccak(lower.as_bytes());

        let mut out = String::with_capacity(42);
        out.push_str("0x");
        for (i, c) in lower.chars().enumerate() {
            let nibble = if i % 2 == 0 {
                hash[i / 2] >> 4
            } else {
                hash[i / 2] & 0x0f
            };
            if c.is_ascii_alphabetic() && nibble >= 8 {
                out.push(c.to_ascii_uppercase());
            } else {
                out.push(c);
            }
        }
        out
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_checksum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // the example addresses from the EIP-55 specification
    const EIP55: [&str; 4] = [
        "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
        "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
        "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
        "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
    ];

    #[test]
    fn checksums_match_the_eip55_test_vectors() {
        for s in EIP55 {
            let addr = Address::parse(&s.to_lowercase()).unwrap();
            assert_eq!(addr.to_checksum(), s);
        }
    }

    #[test]
    fn accepts_valid_checksummed_lowercase_and_uppercase() {
        let addr = Address::parse(EIP55[0]).unwrap();
        assert_eq!(Address::parse(&EIP55[0].to_lowercase()).unwrap(), addr);
        assert_eq!(
            Address::parse(&format!("0x{}", EIP55[0][2..].to_uppercase())).unwrap(),
            addr
        );
    }

    #[test]
    fn rejects_a_wrong_checksum() {
        // flip the case of one letter in a valid checksummed address
        let broken = "0x5AAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
        match Address::parse(broken) {
            Err(ParseAddressError::BadChecksum { expected }) => assert_eq!(expected, EIP55[0]),
            other => panic!("expected a checksum error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(
            Address::parse("5aaeb6053f3e94c9b9a09f33669435e7ef1beaed"),
            Err(ParseAddressError::MissingPrefix)
        );
        assert_eq!(
            Address::parse("0x1234"),
            Err(ParseAddressError::BadLength(4))
        );
        assert_eq!(
            Address::parse("0xzzaeb6053f3e94c9b9a09f33669435e7ef1beaed"),
            Err(ParseAddressError::NotHex)
        );
        assert_eq!(Address::parse(""), Err(ParseAddressError::MissingPrefix));
    }

    #[test]
    fn display_is_checksummed() {
        assert_eq!(
            Address::parse(&EIP55[1].to_lowercase())
                .unwrap()
                .to_string(),
            EIP55[1]
        );
    }
}
