// Regenerates THIRD_PARTY_LICENSES.md: keeps the hand-written sections and rewrites the version line and the Rust crate table
// from `cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-gnu --offline > meta.json`.
// Usage: node scripts/gen-licenses.cjs <meta.json> <app-version>
const fs = require("fs");
const [metaPath, version] = process.argv.slice(2);
if (!metaPath || !version) {
  console.error("usage: node scripts/gen-licenses.cjs <meta.json> <app-version>");
  process.exit(1);
}
const NL = String.fromCharCode(10), CR = String.fromCharCode(13);
const m = JSON.parse(fs.readFileSync(metaPath, "utf8"));
const workspace = new Set(m.workspace_members);
const linked = new Set(m.resolve.nodes.map(n => n.id));
const crates = m.packages
  .filter(p => !workspace.has(p.id) && linked.has(p.id))
  .map(p => [p.name, p.version, String(p.license || "see crate").replace(/\|/g, "/")])
  .sort((a, b) => a[0].localeCompare(b[0]) || a[1].localeCompare(b[1]));

const file = "THIRD_PARTY_LICENSES.md";
let md = fs.readFileSync(file, "utf8").split(CR + NL).join(NL);
md = md.replace(/Trasea \d+\.\d+\.\d+ is MIT licensed\./, `Trasea ${version} is MIT licensed.`);
const i = md.indexOf("## Rust crates linked into the Windows app");
if (i < 0) throw new Error("crate table heading not found");
let out = md.slice(0, i) + `## Rust crates linked into the Windows app (${crates.length})${NL}${NL}| Crate | Version | License |${NL}|---|---|---|${NL}`;
for (const c of crates) out += `| ${c[0]} | ${c[1]} | ${c[2]} |${NL}`;
fs.writeFileSync(file, out);
console.log("crates", crates.length);
console.log("GPL-only:", crates.filter(c => /GPL/.test(c[2]) && !/(MIT|Apache|BSD)/.test(c[2])).map(c => c.join(" ")));
console.log("network:", crates.filter(c => /^(reqwest|hyper|ureq|curl|rustls|openssl|native-tls|tokio-tungstenite)$/.test(c[0])).map(c => c[0]));
console.log("ed25519:", crates.filter(c => /ed25519|curve25519/.test(c[0])).map(c => c[0]));
