# AGENTS.md — Intake Pro v1.0

Intake Pro is a desktop app (macOS and Windows only) forked from
[meetily](https://github.com/Zackriya-Solutions/meetily) (MIT). Product: `README.md`.
Build: `BUILDING.md`. v1.0 architecture and cleanup: `docs/V1_PLAN.md`. Earlier process
records (partly historical): `docs/DEVELOPMENT_PLAN.md`, `docs/REUSE_MAP.md`,
`docs/CHUNK_REPORTS.md`. The original build spec `intake-copilot-build-spec.md` is not in the repo.

## Commands

Run from the repository root unless stated otherwise.

### Analysis core (pure Rust, fast)
```bash
cargo test -p copilot-core
cargo run -p copilot-core --bin copilot-eval -- crates/copilot-core/tests/fixtures/intake_corpus.json
cargo run -p copilot-core --bin copilot-eval -- crates/copilot-core/tests/fixtures/intake_corpus.json --degrade   # proves the harness measures retrieval
```

### Security invariant audit
```bash
node scripts/intake-audit.mjs     # 11 source-level invariants; run in CI
```

### Frontend (from `frontend/`)
```bash
pnpm install
pnpm typecheck        # tsc --noEmit
pnpm lint             # ESLint over the meeting / co-pilot code
pnpm test             # Vitest (src/**/*.test.ts, tests/lib)
pnpm lint:upstream    # whole-tree ESLint (pre-existing debt, not a gate)
node ../scripts/e2e-ui.mjs   # UI flow against a mocked Tauri backend (header explains setup)
```

### Type-check the desktop app without Xcode
```bash
cargo build -p llama-helper
mkdir -p frontend/src-tauri/binaries
cp target/debug/llama-helper frontend/src-tauri/binaries/llama-helper-aarch64-apple-darwin
DOCS_RS=1 cargo check -p intake-pro --lib                    # check-only; never use DOCS_RS for a real build
DOCS_RS=1 cargo check -p intake-pro --lib --features onnx-embed   # ONNX embedding path
```
`DOCS_RS=1` makes the macOS `cidre` build script skip `xcodebuild`. Launching the
app still requires a full Xcode install (`cidre` builds a vendored Xcode project
with `xcodebuild`). Xcode has no Homebrew cask — install it from the App Store or
with `xcodes`, then `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`.

### Analysis pipeline
`copilot-core::analyze_meeting` runs window → embed → hybrid retrieve → prompt →
constrained parse → rubric validate for a whole question set. It runs with the
deterministic `HashingEmbedder` + `HeuristicLlm` and no model, so the pipeline is
exercisable end to end; the app switches to ONNX embeddings (`onnx-embed` feature)
and a loopback `llama-server` / OpenAI-compatible client at runtime.

### Desktop app
```bash
cd frontend && pnpm tauri:dev      # full app (heavy native build)
```

## Global invariants (release blockers)

1. `llama-server` is never running while audio is being captured.
2. No audio file exists outside the designated cache directory.
3. No API key exists outside the OS secret store.
4. No local service accepts a request without the session token.
5. No model loads without a manifest digest match.
6. Live question status is rule-based only and never persisted as a verdict. (Amended for v1.0 at the
   owner's request: co-pilot meetings tag questions live. Invariant 1 still forbids any model
   during capture; `scripts/intake-audit.mjs` enforces that the live path calls no model.)
7. Every automated verdict is user-correctable.
8. Every automated verdict cites transcript evidence.

A change that breaks any of these is a defect regardless of what else it achieves.

## Non-goals

No LLM inference during recording (live question tags use the rule-based analyser only); no meeting bot, cloud transcription, or audio upload; English only; no
ATS/CRM write-back or third-party local API; no multi-user accounts; never add
`torch` or `sentence-transformers`; no vector database.

## Conventions

- Coverage (`QuestionStatus`) and quality (`AnswerRating`) are orthogonal. A rating
  is NULL unless status is `answered`.
- Multi-value fields are JSON arrays. Never split on `,`; import uses one variant
  per row or `|`.
- Status/rating values are exactly `not_asked | asked_not_answered | answered` and
  `Poor | Partial | Strong`.
- Keep diffs minimal. Justify every new dependency and check it against the 8 GB
  envelope.

## Chunk report template

Every chunk completes with this report:

```markdown
## Chunk <n> — <name> — Agent: <AGENT>

### Reuse decision
Existing code found: <paths>
Sufficient? <yes/no, why>
Action taken: <use-as-is | configure | adapt | modify | new>
Justification for any new file: <one line each>

### New dependencies
<name> — <why> — <size/RAM impact>   (or "none")

### Changes
Modified: <paths>
Created:  <paths>

### Verification gate
<checklist, PASS/FAIL with evidence. "Looks correct" is not evidence.>

### Measurements
Peak RSS: <n> MB    Wall time: <n>    Reference hardware: <spec>

### Deviations from spec
<what, why, impact>   (or "none")

### Blocked / needs human input
<what, why>   (or "none")
```
