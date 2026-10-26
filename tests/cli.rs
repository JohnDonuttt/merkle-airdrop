use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const A: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
const B: &str = "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359";
const C: &str = "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB";

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_merkle-airdrop"))
}

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("merkle-airdrop-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn build(dir: &Path) -> PathBuf {
    let csv = dir.join("airdrop.csv");
    fs::write(
        &csv,
        format!("address,amount\n{A},100\n{B},250\n{C},1000000000000000000\n"),
    )
    .unwrap();
    let out = dir.join("tree.json");
    let res = bin()
        .args(["build", csv.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(res.status.success(), "{}", text(&res.stderr));
    out
}

fn ok(out: &Output) -> String {
    assert!(out.status.success(), "stderr: {}", text(&out.stderr));
    text(&out.stdout)
}

#[test]
fn build_writes_a_distribution_and_summarises_on_stderr() {
    let dir = scratch("build");
    let csv = dir.join("a.csv");
    fs::write(&csv, format!("address,amount\n{A},100\n{B},250\n")).unwrap();
    let out_path = dir.join("tree.json");

    let res = bin()
        .args([
            "build",
            csv.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(res.status.success());
    let log = text(&res.stderr);
    assert!(log.contains("2 claims, total 350"), "{log}");
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&out_path).unwrap()).unwrap();
    assert!(json["merkleRoot"].as_str().unwrap().starts_with("0x"));
    assert_eq!(json["claims"].as_array().unwrap().len(), 2);
}

#[test]
fn build_without_output_prints_json_to_stdout() {
    let dir = scratch("stdout");
    let csv = dir.join("a.csv");
    fs::write(&csv, format!("address,amount\n{A},1\n")).unwrap();

    let stdout = ok(&bin()
        .args(["build", csv.to_str().unwrap()])
        .output()
        .unwrap());
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["tokenTotal"], "1");
}

#[test]
fn same_input_always_gives_the_same_file() {
    let a = fs::read_to_string(build(&scratch("det1"))).unwrap();
    let b = fs::read_to_string(build(&scratch("det2"))).unwrap();
    assert_eq!(a, b);
}

#[test]
fn check_accepts_a_built_file_and_rejects_a_tampered_one() {
    let dir = scratch("check");
    let tree = build(&dir);

    let out = bin()
        .args(["check", tree.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(ok(&out).starts_with("ok: 3 claims verify"));

    let tampered = fs::read_to_string(&tree).unwrap().replacen(
        "\"amount\": \"100\"",
        "\"amount\": \"999\"",
        1,
    );
    assert_ne!(
        tampered,
        fs::read_to_string(&tree).unwrap(),
        "test setup should have changed the file"
    );
    let bad = dir.join("tampered.json");
    fs::write(&bad, tampered).unwrap();

    let out = bin()
        .args(["check", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("does not verify"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn proof_prints_a_claim_that_then_verifies_standalone() {
    let dir = scratch("proof");
    let tree = build(&dir);

    // lowercase input still finds the checksummed entry
    let out = bin()
        .args(["proof", tree.to_str().unwrap(), &B.to_lowercase()])
        .output()
        .unwrap();
    let claim: serde_json::Value = serde_json::from_str(&ok(&out)).unwrap();
    assert_eq!(claim["amount"], "250");

    let root = serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&tree).unwrap())
        .unwrap()["merkleRoot"]
        .as_str()
        .unwrap()
        .to_string();
    let proof: Vec<String> = claim["proof"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_string())
        .collect();

    let good = bin()
        .args([
            "verify",
            "--root",
            &root,
            "--address",
            B,
            "--amount",
            "250",
            "--proof",
            &proof.join(","),
        ])
        .output()
        .unwrap();
    assert_eq!(ok(&good).trim(), "valid");

    let wrong_amount = bin()
        .args([
            "verify",
            "--root",
            &root,
            "--address",
            B,
            "--amount",
            "251",
            "--proof",
            &proof.join(","),
        ])
        .output()
        .unwrap();
    assert!(!wrong_amount.status.success());
}

#[test]
fn proof_for_an_address_not_in_the_file_fails_cleanly() {
    let dir = scratch("missing");
    let tree = build(&dir);
    let out = bin()
        .args([
            "proof",
            tree.to_str().unwrap(),
            "0x0000000000000000000000000000000000000001",
        ])
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("no claim for address"));
}

#[test]
fn bad_csv_is_reported_with_a_row_number_and_exits_nonzero() {
    let dir = scratch("badcsv");
    let csv = dir.join("bad.csv");
    fs::write(&csv, format!("address,amount\n{A},100\n{B},ten\n")).unwrap();

    let out = bin()
        .args(["build", csv.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("row 3"), "{}", text(&out.stderr));
    assert!(
        out.stdout.is_empty(),
        "nothing should be written on failure"
    );
}

#[test]
fn duplicate_addresses_are_refused() {
    let dir = scratch("dup");
    let csv = dir.join("dup.csv");
    fs::write(&csv, format!("address,amount\n{A},100\n{A},200\n")).unwrap();

    let out = bin()
        .args(["build", csv.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("more than once"));
}

#[test]
fn missing_file_is_a_clean_error_not_a_panic() {
    let out = bin()
        .args(["check", "/definitely/not/here.json"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(text(&out.stderr).starts_with("error:"));
    assert!(!text(&out.stderr).contains("panicked"));
}
