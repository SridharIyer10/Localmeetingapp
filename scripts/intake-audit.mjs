#!/usr/bin/env node
// Intake Pro security-invariant audit (Chunk 10, verifiable part).
//
// This does not replace the full manual audit or the cross-origin runtime test;
// it is a fast, repeatable gate for the invariants that can be checked from the
// source tree. Run from the repository root: node scripts/intake-audit.mjs

import fs from "node:fs";
import path from "node:path";

const ROOT = process.cwd();
const results = [];

function read(file) {
  try {
    return fs.readFileSync(path.join(ROOT, file), "utf8");
  } catch {
    return null;
  }
}

function walk(dir, exts, out = []) {
  const abs = path.join(ROOT, dir);
  if (!fs.existsSync(abs)) return out;
  for (const e of fs.readdirSync(abs, { withFileTypes: true })) {
    const p = path.join(abs, e.name);
    if (e.isDirectory()) walk(path.relative(ROOT, p), exts, out);
    else if (exts.some((x) => e.name.endsWith(x))) out.push(path.relative(ROOT, p));
  }
  return out;
}

function check(name, ok, detail) {
  results.push({ name, ok: Boolean(ok), detail });
}

const copilotFiles = [
  ...walk("frontend/src/components/copilot", [".ts", ".tsx"]),
  ...walk("frontend/src/components/meeting", [".ts", ".tsx"]),
  ...walk("frontend/src/lib/copilot", [".ts", ".tsx"]),
  ...walk("frontend/src/lib/meeting", [".ts", ".tsx"]),
];
const copilotSource = copilotFiles.map((f) => read(f) ?? "").join("\n");

// 1. No API key columns in the copilot settings table.
const migration = read("frontend/src-tauri/migrations/20260115000000_add_intake_copilot.sql") ?? "";
check(
  "no api-key columns in ai_settings",
  !/api_key/i.test(migration),
  "ai_settings must not contain key columns",
);

// 2. No forbidden ML frameworks anywhere.
const manifests = [
  "Cargo.toml",
  "frontend/src-tauri/Cargo.toml",
  "crates/copilot-core/Cargo.toml",
  "llama-helper/Cargo.toml",
  "frontend/package.json",
];
const manifestText = manifests.map((f) => read(f) ?? "").join("\n");
check(
  "no torch / sentence-transformers",
  !/sentence-transformers|\btorch\b/i.test(manifestText),
  "embeddings run on ONNX Runtime only",
);

// 3. No unsafe HTML on the copilot path.
check(
  "no dangerouslySetInnerHTML on copilot path",
  !/dangerouslySetInnerHTML/.test(copilotSource),
  "report text must render as text",
);

// 4. Manual tagging only during the call — no live detection or model evaluation.
check(
  "no similarity/matching code on the copilot frontend",
  !/\bcosine\s*\(|similarity\s*\(|similarityScore|detectQuestion|copilot_live_status|\bliveStatus\b/i.test(copilotSource),
  "during the call, question tags are manual user inputs only",
);
const analysisRs = read("frontend/src-tauri/src/copilot/analysis.rs") ?? "";
const commandsRs = read("frontend/src-tauri/src/copilot/commands.rs") ?? "";
check(
  "no live question evaluation in backend",
  !/\blive_statuses\b|\bcopilot_live_status\b/.test(analysisRs + "\n" + commandsRs),
  "no live evaluation runs during capture; all compute goes to transcription",
);

// 5. Secrets live in the OS store, not the DB/config.
check(
  "secret store abstraction present",
  /pub trait SecretStore/.test(read("frontend/src-tauri/src/copilot/secret_store.rs") ?? ""),
  "API keys go to Keychain/DPAPI",
);

// 6. Local service auth is loopback-only and denies browser origins.
const auth = read("crates/copilot-core/src/local_auth.rs") ?? "";
check(
  "local service binds loopback only",
  /Ipv4Addr::LOCALHOST/.test(auth),
  "127.0.0.1 ephemeral bind",
);
check(
  "local service denies browser origins",
  /fn is_allowed_origin/.test(auth) && /origin\.is_none\(\)/.test(auth),
  "deny all CORS origins",
);

// 7. Model digest verification exists and fails closed.
const manifest = read("crates/copilot-core/src/model_manifest.rs") ?? "";
check(
  "model digest verified before load",
  /pub fn verify_model/.test(manifest) && /DigestMismatch/.test(manifest),
  "no model loads without a manifest digest match",
);

// 8. Foreign keys + WAL enforced on every connection.
const manager = read("frontend/src-tauri/src/database/manager.rs") ?? "";
check(
  "foreign_keys + WAL enforced",
  /foreign_keys\(true\)/.test(manager) && /SqliteJournalMode::Wal/.test(manager),
  "cascade deletes and live writes require both pragmas",
);

// 9. Slidecar is gated off during capture.
const gate = read("crates/copilot-core/src/lifecycle_gate.rs") ?? "";
check(
  "sidecar cannot run while capturing",
  /pub fn sidecar_allowed/.test(gate) && /!capturing/.test(gate),
  "llama-server never runs during capture",
);

// 10. No audio path outside the designated cache (copilot code).
check(
  "audio chunked + encrypted",
  /pub fn chunk_pcm/.test(read("crates/copilot-core/src/audio_chunk.rs") ?? "") &&
    /XChaCha20Poly1305/.test(read("crates/copilot-core/src/audio_crypto.rs") ?? ""),
  "VAD-aligned chunks, encrypted at rest",
);

// 11. One version, defined once (workspace Cargo.toml); nothing else hard-codes one.
const wsVersion = /\[workspace\.package\][^\[]*?\nversion = "([^"]+)"/.exec(read("Cargo.toml") ?? "")?.[1];
const pkgVersion = JSON.parse(read("frontend/package.json") ?? "{}").version;
const tauriConf = JSON.parse(read("frontend/src-tauri/tauri.conf.json") ?? "{}");
const inherits = ["frontend/src-tauri/Cargo.toml", "llama-helper/Cargo.toml", "crates/copilot-core/Cargo.toml"].every(
  (f) => /^version\.workspace = true/m.test(read(f) ?? ""),
);
check(
  "single version string",
  wsVersion && wsVersion === pkgVersion && !("version" in tauriConf) && inherits,
  `workspace=${wsVersion} package.json=${pkgVersion} tauri.conf version=${tauriConf.version ?? "(none, good)"} crates inherit=${inherits}`,
);

let failed = 0;
for (const r of results) {
  if (!r.ok) failed++;
  console.log(`${r.ok ? "PASS" : "FAIL"}  ${r.name}${r.ok ? "" : `  (${r.detail})`}`);
}
console.log(`\n${results.length - failed}/${results.length} invariant checks passed`);
process.exit(failed === 0 ? 0 : 1);
