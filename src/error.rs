use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseAddressError {
    #[error("address must start with 0x")]
    MissingPrefix,
    #[error("address must be 40 hex characters after 0x, got {0}")]
    BadLength(usize),
    #[error("address contains a non-hex character")]
    NotHex,
    #[error("mixed-case address has a bad EIP-55 checksum (expected {expected})")]
    BadChecksum { expected: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseAmountError {
    #[error("amount is empty")]
    Empty,
    #[error("amount must be a whole number of base units (digits only)")]
    NotADecimal,
    #[error("amount does not fit in a uint256")]
    Overflow,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("row {row}: invalid address {value:?}: {source}")]
    Address {
        row: usize,
        value: String,
        source: ParseAddressError,
    },

    #[error("row {row}: invalid amount {value:?}: {source}")]
    Amount {
        row: usize,
        value: String,
        source: ParseAmountError,
    },

    #[error("row {row}: amount is zero")]
    ZeroAmount { row: usize },

    #[error("address {0} appears more than once")]
    DuplicateAddress(String),

    #[error("the input has no claims")]
    Empty,

    #[error("the CSV needs a header row with `address` and `amount` columns")]
    MissingColumns,

    #[error("the total of all amounts does not fit in a uint256")]
    TotalOverflow,

    #[error("claim for {address} does not verify against the root")]
    BadProof { address: String },

    #[error("root in the file is {file}, but rebuilding the tree gives {rebuilt}")]
    RootMismatch { file: String, rebuilt: String },

    #[error("tokenTotal in the file is {file}, but the claims add up to {computed}")]
    TotalMismatch { file: String, computed: String },

    #[error("invalid hash {0:?}: expected 0x followed by 64 hex characters")]
    BadHash(String),

    #[error("no claim for address {0}")]
    NotFound(String),

    #[error(transparent)]
    Csv(#[from] csv::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
