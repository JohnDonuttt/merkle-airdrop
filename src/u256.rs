use crate::error::ParseAmountError;
use std::fmt;

/// A 256-bit unsigned integer stored big-endian, which is exactly how the EVM encodes a uint256.
/// Only the operations an airdrop needs: parse, add, print.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct U256(pub [u8; 32]);

impl U256 {
    pub const ZERO: U256 = U256([0; 32]);

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|b| *b == 0)
    }

    /// Parse a base-10 string. Rejects signs, decimals, separators and anything over 2^256 - 1.
    pub fn from_decimal(s: &str) -> Result<Self, ParseAmountError> {
        if s.is_empty() {
            return Err(ParseAmountError::Empty);
        }
        if !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ParseAmountError::NotADecimal);
        }

        let mut out = [0u8; 32];
        for digit in s.bytes().map(|b| (b - b'0') as u16) {
            // out = out * 10 + digit, least-significant byte first
            let mut carry = digit;
            for byte in out.iter_mut().rev() {
                let v = (*byte as u16) * 10 + carry;
                *byte = (v & 0xff) as u8;
                carry = v >> 8;
            }
            if carry != 0 {
                return Err(ParseAmountError::Overflow);
            }
        }
        Ok(U256(out))
    }

    pub fn checked_add(self, other: U256) -> Option<U256> {
        let mut out = [0u8; 32];
        let mut carry = 0u16;
        for i in (0..32).rev() {
            let v = self.0[i] as u16 + other.0[i] as u16 + carry;
            out[i] = (v & 0xff) as u8;
            carry = v >> 8;
        }
        (carry == 0).then_some(U256(out))
    }
}

impl fmt::Display for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return f.write_str("0");
        }

        // repeated division by 10, collecting remainders
        let mut n = self.0;
        let mut digits = Vec::with_capacity(78);
        while n.iter().any(|b| *b != 0) {
            let mut rem = 0u16;
            for byte in n.iter_mut() {
                let cur = rem * 256 + *byte as u16;
                *byte = (cur / 10) as u8;
                rem = cur % 10;
            }
            digits.push(b'0' + rem as u8);
        }
        digits.reverse();
        f.write_str(std::str::from_utf8(&digits).expect("digits are ascii"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: &str =
        "115792089237316195423570985008687907853269984665640564039457584007913129639935";

    #[test]
    fn parses_small_numbers() {
        assert_eq!(U256::from_decimal("0").unwrap(), U256::ZERO);
        let mut one = [0u8; 32];
        one[31] = 1;
        assert_eq!(U256::from_decimal("1").unwrap(), U256(one));

        let mut v = [0u8; 32];
        v[30] = 0x01;
        v[31] = 0x00;
        assert_eq!(U256::from_decimal("256").unwrap(), U256(v));
    }

    #[test]
    fn accepts_max_and_rejects_one_more() {
        assert_eq!(U256::from_decimal(MAX).unwrap(), U256([0xff; 32]));
        let over = "115792089237316195423570985008687907853269984665640564039457584007913129639936";
        assert_eq!(U256::from_decimal(over), Err(ParseAmountError::Overflow));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(U256::from_decimal(""), Err(ParseAmountError::Empty));
        for bad in ["-1", "+1", "1.5", "1e18", "1,000", " 1", "0x10", "abc"] {
            assert_eq!(
                U256::from_decimal(bad),
                Err(ParseAmountError::NotADecimal),
                "{bad}"
            );
        }
    }

    #[test]
    fn leading_zeros_are_fine_and_canonicalised() {
        assert_eq!(U256::from_decimal("000123").unwrap().to_string(), "123");
        assert_eq!(U256::from_decimal("000").unwrap().to_string(), "0");
    }

    #[test]
    fn display_round_trips() {
        for s in [
            "0",
            "1",
            "9",
            "10",
            "255",
            "256",
            "1000000000000000000",
            "123456789012345678901234567890",
            MAX,
        ] {
            assert_eq!(U256::from_decimal(s).unwrap().to_string(), s);
        }
    }

    #[test]
    fn adds_with_carry_and_detects_overflow() {
        let a = U256::from_decimal("255").unwrap();
        let b = U256::from_decimal("1").unwrap();
        assert_eq!(a.checked_add(b).unwrap().to_string(), "256");

        let max = U256::from_decimal(MAX).unwrap();
        assert!(max.checked_add(b).is_none());
        assert_eq!(max.checked_add(U256::ZERO), Some(max));
    }
}
