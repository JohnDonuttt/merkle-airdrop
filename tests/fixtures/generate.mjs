// Regenerates oz_vectors.json from OpenZeppelin's reference JS implementation.
//   npm install @openzeppelin/merkle-tree   (the committed vectors used 1.0.8)
//   node generate.mjs
import { StandardMerkleTree } from "@openzeppelin/merkle-tree";
import { writeFileSync } from "node:fs";

// deterministic pseudo-addresses so the vectors are reproducible
const addr = (n) => "0x" + n.toString(16).padStart(40, "0");
const sets = {
  single: [[addr(0xa1), "1000"]],
  two: [[addr(0xa1), "1000"], [addr(0xb2), "2500"]],
  three: [[addr(1), "5"], [addr(2), "10"], [addr(3), "15"]],
  five_big_amounts: [
    [addr(0x11), "1000000000000000000"],
    [addr(0x22), "123456789012345678901234567890"],
    [addr(0x33), "1"],
    [addr(0x44), "115792089237316195423570985008687907853269984665640564039457584007913129639935"],
    [addr(0x55), "42"],
  ],
  eight: Array.from({ length: 8 }, (_, i) => [addr(0x1000 + i * 7919), String((i + 1) * 100)]),
  eleven: Array.from({ length: 11 }, (_, i) => [addr(0xabc000 + i * 104729), String(10 ** (i % 6) + i)]),
};

const out = {};
for (const [name, values] of Object.entries(sets)) {
  const tree = StandardMerkleTree.of(values, ["address", "uint256"]);
  const claims = values.map(([address, amount], i) => ({ address, amount, proof: tree.getProof(i) }));
  out[name] = { root: tree.root, claims };
}
writeFileSync("oz_vectors.json", JSON.stringify(out, null, 2) + "\n");
console.log("sets:", Object.keys(out).join(", "));
for (const [k, v] of Object.entries(out)) console.log(k.padEnd(18), v.root, `(${v.claims.length} claims, max proof ${Math.max(...v.claims.map(c => c.proof.length))})`);
