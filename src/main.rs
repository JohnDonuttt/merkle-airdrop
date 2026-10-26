use clap::{Parser, Subcommand};
use merkle_airdrop::distribution::{hash_from_hex, parse_csv, Claim};
use merkle_airdrop::{leaf_hash, verify, Address, Distribution, Error, Result, U256};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "merkle-airdrop",
    version,
    about = "Build and verify Merkle trees for token airdrops"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build a distribution (root + a proof per recipient) from a CSV of `address,amount`
    Build {
        /// CSV with an `address,amount` header; amounts are whole base units (e.g. wei)
        input: PathBuf,
        /// Write the JSON here instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Audit a distribution file: every proof, the root and the total must all be consistent
    Check { distribution: PathBuf },
    /// Print one recipient's claim (amount and proof) from a distribution file
    Proof {
        distribution: PathBuf,
        address: String,
    },
    /// Verify a proof without a distribution file, the way a recipient or contract would
    Verify {
        #[arg(long)]
        root: String,
        #[arg(long)]
        address: String,
        #[arg(long)]
        amount: String,
        /// Proof hashes, in order
        #[arg(long, num_args = 0.., value_delimiter = ',')]
        proof: Vec<String>,
    },
}

fn load(path: &PathBuf) -> Result<Distribution> {
    Ok(serde_json::from_reader(File::open(path)?)?)
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Build { input, output } => {
            let claims = parse_csv(File::open(&input)?)?;
            let dist = Distribution::build(claims)?;
            let json = serde_json::to_string_pretty(&dist)? + "\n";

            match output {
                Some(path) => {
                    fs::write(&path, json)?;
                    eprintln!("root {}", dist.merkle_root);
                    eprintln!("{} claims, total {}", dist.claims.len(), dist.token_total);
                    eprintln!("wrote {}", path.display());
                }
                None => io::stdout().write_all(json.as_bytes())?,
            }
        }

        Command::Check { distribution } => {
            let dist = load(&distribution)?;
            let n = dist.check()?;
            println!("ok: {n} claims verify against root {}", dist.merkle_root);
        }

        Command::Proof {
            distribution,
            address,
        } => {
            let dist = load(&distribution)?;
            let addr = Address::parse(&address).map_err(|source| Error::Address {
                row: 0,
                value: address.clone(),
                source,
            })?;
            let claim: &Claim = dist
                .find(&addr)
                .ok_or_else(|| Error::NotFound(addr.to_string()))?;
            println!("{}", serde_json::to_string_pretty(claim)?);
        }

        Command::Verify {
            root,
            address,
            amount,
            proof,
        } => {
            let root = hash_from_hex(&root)?;
            let addr = Address::parse(&address).map_err(|source| Error::Address {
                row: 0,
                value: address.clone(),
                source,
            })?;
            let amount = U256::from_decimal(&amount).map_err(|source| Error::Amount {
                row: 0,
                value: amount.clone(),
                source,
            })?;
            let proof = proof
                .iter()
                .map(|p| hash_from_hex(p))
                .collect::<Result<Vec<_>>>()?;

            if verify(&root, &leaf_hash(&addr, &amount), &proof) {
                println!("valid");
            } else {
                return Err(Error::BadProof {
                    address: addr.to_string(),
                });
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
