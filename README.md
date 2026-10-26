# merkle-airdrop

A command-line tool and Rust library for building the Merkle tree behind a token airdrop. Give it a CSV of `address,amount` and it produces the **root** to put in your contract and a **proof** for every recipient to claim with.

The output is compatible with [OpenZeppelin's `StandardMerkleTree`](https://github.com/OpenZeppelin/merkle-tree) and verifies with `MerkleProof.verify` in Solidity.

```sh
$ merkle-airdrop build airdrop.csv -o tree.json
root 0xdf5f05c7c9196b95b2ca91336f3648ede95064bc09067f8f1bd0252e4de72f8a
25 claims, total 2697472606172500791809067697869
wrote tree.json
```

## Install

Needs a recent stable Rust toolchain.

```sh
cargo install --path .
# or just: cargo run --release -- <command>
```

## Usage

**Input:** a CSV with an `address,amount` header (any column order). Amounts are whole base units (wei), digits only.

```csv
address,amount
0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed,100
0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359,250
0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB,1000000000000000000
```

| Command | What it does |
| --- | --- |
| `build <csv> [-o out.json]` | Build the tree. Writes JSON to `-o`, or stdout if omitted |
| `check <tree.json>` | Audit a file: every proof, the root and the total must be consistent |
| `proof <tree.json> <address>` | Print one recipient's claim (amount and proof) |
| `verify --root R --address A --amount N --proof h1,h2,...` | Verify a claim without the file, like a recipient would |

**Output** (`tree.json`):

```json
{
  "merkleRoot": "0x…",
  "tokenTotal": "1000000000000000350",
  "claims": [
    { "address": "0x5aAe…", "amount": "100", "proof": ["0x…", "0x…"] }
  ]
}
```

Publish the file (or serve each recipient's `claims` entry from an API), fund the contract with at least `tokenTotal`, and set `merkleRoot` in the contract.

## Using the root on-chain

```solidity
import {MerkleProof} from "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";

function claim(address account, uint256 amount, bytes32[] calldata proof) external {
    require(!claimed[account], "already claimed");
    bytes32 leaf = keccak256(bytes.concat(keccak256(abi.encode(account, amount))));
    require(MerkleProof.verify(proof, root, leaf), "bad proof");
    claimed[account] = true;
    token.transfer(account, amount);
}
```

The leaf is `keccak256(keccak256(abi.encode(address, uint256)))`. The double hash is deliberate: it stops a 64-byte leaf from being passed off as an internal node (a second-preimage attack).

## What it checks

It refuses to build a tree from bad input instead of producing something that looks fine:

- **Checksum typos.** A mixed-case address must carry a valid EIP-55 checksum. All-lowercase or all-uppercase is accepted.
- **Duplicate addresses.** Two rows for one address is an error, since one could claim twice or shadow the other.
- **Zero amounts, decimals, signs and separators** (`1.5`, `1e18`, `1,000`, `-1`) are rejected, with the row number.
- **Overflow.** Amounts must fit in a `uint256`, and so must their total.
- **Row numbers** in every error: `row 3: invalid amount "ten": ...`.
- The same CSV always produces the same file, regardless of row order.

## How it is built

- The tree has the same layout as OpenZeppelin's: leaves sorted by hash, stored in a flat array with no padding or duplicated leaves, and sibling pairs hashed in sorted order. That is why proofs are plain lists of hashes with no left/right flags.
- Keccak-256 is the original Keccak used by Ethereum, not NIST SHA3-256. They give different digests.
- `U256` is a small hand-rolled 256-bit integer (parse, add, print) rather than a bignum dependency. Dependencies are `clap`, `csv`, `serde`, `serde_json`, `hex`, `thiserror` and `tiny-keccak`.

## Testing

```sh
cargo test
```

56 tests:

- **Reference vectors (4).** Roots and proofs are compared byte for byte with OpenZeppelin's JavaScript implementation, over six input shapes: 1, 2, 3, 5, 8 and 11 leaves, including an amount of 2^256 − 1. The vectors are in `tests/fixtures`, with the script that regenerates them.
- **Property tests (8).** Over random inputs: every proof verifies, the root doesn't depend on input order, any changed amount fails to verify, proofs are at most ⌈log₂ n⌉ long, `hash_pair` is commutative, and U256 decimal output round-trips.
- **CLI tests (9)** run the real binary: build, check, tampered files, proof lookup, standalone verify, bad CSVs, duplicates, missing files.
- **Unit tests (35)**, including the EIP-55 test vectors from the specification.

I also built a 25-recipient airdrop with this tool and verified every claim on an EVM using OpenZeppelin's `MerkleProof` in a Foundry test. All 25 claims verified. A wrong amount, someone else's proof and a second claim all reverted. That scratch project is not included here.

## Limitations

- ERC-20-style `(address, uint256)` leaves only. Other leaf shapes would need a different encoding.
- It holds the whole tree in memory, which suits typical airdrops. I haven't benchmarked very large ones.
- It doesn't deploy contracts or sign anything. It produces the data you give to your contract.
- No multi-chain or per-token handling: one file is one distribution.

## License

MIT
