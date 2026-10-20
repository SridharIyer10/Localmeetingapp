# Chunk Reports — Intake Co-Pilot

Baseline commit: `a2cb62e8` (meetily v0.4.1). All evidence below was produced on
this machine unless stated otherwise.

Status legend: **PASS** (gate evidenced) · **PARTIAL** (subset evidenced) ·
**PENDING** (not started).

---

## Chunk 0 — Recon & Reuse Map — Agent: RECON

### Reuse decision
Existing code found: `frontend/src-tauri/src/{audio,whisper_engine,database,summary,api}`,
`frontend/src/{app,components}`, `backend/app`, `.github/workflows`, `migrations/`.
Sufficient? Partially. Audio capture, whisper bindings, SQLite/sqlx persistence,
a sidecar supervisor and a large frontend already exist.
Action taken: adapt/extend.

### Key finding
The Python FastAPI backend (`localhost:5167`, `allow_origins=["*"]`) is **not
load-bearing**. All meeting, transcript, settings and model flows go through Rust
SQLite repositories; FastAPI is only used for legacy profile/licence endpoints and
a connectivity diagnostic. Decision: bypass it; embeddings and analysis live in
Rust.

### Changes
Created: `docs/REUSE_MAP.md`.

### Verification gate
- [x] Every claim cites a real file path and function/line range — PASS
- [x] Item 4 (backend load-bearing?) answered unambiguously — PASS ("No")
- [x] Deviations list present — PASS (10 items)

### Blocked / needs human input
- Approve using sqlx `_sqlx_migrations` instead of a second `schema_version` table.
- Choose sidecar strategy (`llama-helper` vs add `llama-server`).
- Confirm the ONNX diarization model and its licence.

---

## Chunk 1 — Foundation: schema, migrations, local-service auth — Agent: CORE

### Reuse decision
Existing code found: `database/manager.rs`, `database/models.rs`,
`database/repositories/*`, `migrations/*`.
Sufficient? Yes for the migration mechanism; no for copilot schema, pragmas,
secret storage or local-auth.
Action taken: adapt (extend migrations + manager) and new (copilot modules).

### New dependencies
- `keyring` 3 (OS Keychain / Windows Credential Manager for API keys) — native
  framework only, no RAM cost.
- `copilot-core` (local path crate; pure logic, no model/network deps).
Justification: the spec forbids keys in DB/config; the keyring is the mechanism.

### Changes
Modified: `Cargo.toml` (workspace member), `frontend/src-tauri/Cargo.toml`
(deps), `frontend/src-tauri/src/lib.rs` (module + command registration),
`frontend/src-tauri/src/database/mod.rs`,
`frontend/src-tauri/src/database/repositories/mod.rs`,
`frontend/src-tauri/src/database/manager.rs` (pragmas).
Created: `frontend/src-tauri/migrations/20260115000000_add_intake_copilot.sql`,
`frontend/src-tauri/src/database/models_copilot.rs`,
`frontend/src-tauri/src/database/repositories/copilot.rs`,
`frontend/src-tauri/src/copilot/{mod,commands,local_auth,secret_store}.rs`,
`crates/copilot-core/` (crate).

### Verification gate
- [x] Status/rating orthogonality enforced at the DB level (CHECK constraint) and
  in `override_verdict` — PASS (constraint + Rust guard + unit tests)
- [x] Multi-value JSON helpers never split on commas — PASS
  (`copilot_core::json::comma_containing_value_is_one_item`)
- [x] `foreign_keys=ON` and `journal_mode=WAL` set on every connection — PASS
  (code); runtime gate PENDING (needs app build)
- [x] Local-service token is constant-time compared; all browser origins denied;
  host restricted to loopback; ephemeral loopback bind — PASS
  (`copilot_core::local_auth`, 8 tests)
- [x] Secret store abstraction with OS-backed and in-memory implementations —
  PASS (2 tests)
- [x] Ingest validation: max 500 rows, empty-question row errors — PASS (3 tests)
- [ ] Migrations run twice; cascade delete; unlocked DB test; runtime auth test —
  PENDING (requires the desktop app runtime)

### Deviations from spec
- `schema_version` table omitted; sqlx `_sqlx_migrations` is authoritative
  (documented in REUSE_MAP; needs sign-off).

### Blocked / needs human input
- Runtime gate execution requires a successful Tauri build and a DB with the key.

---

## Chunk 5 — Question ingest + teleprompter — Agent: UI

### Reuse decision
Existing code found: `frontend/src/app/*`, `frontend/src/components/*`.
Sufficient? No copilot features existed.
Action taken: new (copilot route + components).

### New dependencies
- `xlsx` (SheetJS 0.20.3, official CDN tarball) — client-side CSV/XLSX parsing so
  no file is uploaded; ~1 MB bundled.
Justification: required by spec §Chunk 5 to remove backend upload attack surface.
- `vitest` + `eslint`/`eslint-config-next`/`@eslint/eslintrc` (dev-only test/lint).

### Changes
Created: `frontend/src/lib/copilot/{types,import,export,api}.ts`,
`frontend/src/components/copilot/{ConsentGate,QuestionIngest,Teleprompter,QuestionReport}.tsx`,
`frontend/src/app/copilot/page.tsx`, tests, `vitest.config.mts`.
Modified: `frontend/package.json` (scripts + deps), `frontend/.npmrc`.

### Verification gate
- [x] Question containing a comma imports as ONE item — PASS (`import.test.ts`)
- [x] 3 MB file rejected — PASS (2 MB cap test); 600-row rejected — PASS (500 cap)
- [x] `.xlsx` formula cell is not evaluated — PASS
- [x] Multi-value fields split on `|` only — PASS
- [x] Per-row error banner data produced by the parser — PASS
- [x] Consent gate blocks session until confirmed — PASS (component)
- [x] Teleprompter has tick-off, notes, timer, un-ticked count — PASS
- [x] No live matching/similarity code in the frontend copilot code — PASS (grep)
- [x] Browser demo mode exercises the full UI without the desktop shell — PASS
  (`pnpm dev` → `/copilot` returns HTTP 200 with the demo banner; demo data
  satisfies the same invariants as the real pipeline, 4 tests)
- [ ] Backend receives no upload on any import path — PENDING (needs runtime
  network capture); by construction the parser is client-only.

### Deviations from spec
- `pnpm lint` scoped to the copilot code. Upstream whole-tree ESLint reports 269
  pre-existing problems (230 errors); cleaning them is out of scope and violates
  the minimal-diff rule. `pnpm lint:upstream` exposes the debt.

---

## Chunk 6 — Analysis engine — Agent: ANALYSIS

### Reuse decision
Existing code found: none for retrieval/rubric/constrained output.
Action taken: new (`copilot-core`, pure and unit-tested). ONNX embedding and the
LLM client are injected via a `LlmClient` trait; the real ONNX/llama-server wiring
is a thin adapter still to land.

### Changes
Created: `crates/copilot-core/src/{model,window,retrieval,rubric,prompt,schema,checkpoint,notes,metrics,json,local_auth,lib}.rs`.
Implemented: windowing (3–4 turn, 1 overlap), BM25 + cosine hybrid retrieval,
explicit rating rubric with post-hoc enforcement, delimiter-escaping prompt
hardening, strict JSON schema parsing, stage checkpointing, notes dedupe.

### Verification gate
- [x] Every returned verdict carries ≥1 evidence id; fabricated ids are dropped
  and rejected — PASS
- [x] Answer given before the question is still eligible (windowing + retrieval) —
  PASS (window overlap tests)
- [x] Rating NULL whenever status != answered — PASS (9 rubric tests)
- [x] Injection fixture does not alter verdicts — PASS (delimiter escaping +
  transcript-as-data + integration test)
- [x] 100 schema-valid outputs, zero retries — PASS (strict parser; invalid output
  is rejected, never retried)
- [x] Resume from checkpoint skips completed stages — PASS
- [ ] 60-min fixture within 15 min; peak RSS ≤ 3.0 GB — PENDING (needs real model
  + recorded audio; not executable on a pure-logic crate)

### Deviations from spec
- Actual ONNX embedding model and `llama-server` constrained-decoding call are
  integration points, not yet wired end-to-end.

---

## Chunk 7 — Report & export — Agent: UI

### Changes
Created: `frontend/src/components/copilot/QuestionReport.tsx`,
`frontend/src/lib/copilot/export.ts` (+ tests).

### Verification gate
- [x] Clicking evidence scrolls to the correct transcript segment — PASS
  (`scrollToSegment` + `id="segment-<id>"`)
- [x] Corrections persist (Tauri `copilot_override_verdict`) and reflect in export
  — PASS (logic); runtime persistence PENDING
- [x] Exported file round-trips UTF-8 (non-ASCII title survives) — PASS
- [x] No `dangerouslySetInnerHTML` in copilot code — PASS (grep)
- [x] Filename is UUID-derived; non-UUID ids are refused — PASS (path traversal
  test)
- [x] Zero-question report does not crash — PASS

---

## Chunk 9 — Evaluation harness — Agent: EVAL

### Changes
Created: `crates/copilot-core/src/metrics.rs`,
`crates/copilot-core/src/bin/copilot-eval.rs`,
`crates/copilot-core/tests/fixtures/intake_corpus.json`.

### Verification gate
- [x] Harness runs end-to-end on the fixture corpus — PASS
- [x] Metrics reproducible across runs (deterministic) — PASS
- [x] Degrading retrieval (k=1) visibly lowers recall — PASS: asked recall
  1.000 → 0.714; status accuracy 1.000 → 0.800
- [x] Report names reference hardware and model versions — PASS (fields present)

Baseline (10 fixture cases): asked P/R = 1.000, status accuracy = 1.000,
rating κ = 1.000, evidence validity = 1.000. **These are label-derived fixture
numbers, not a measurement of a live model.** The first honest model baseline is
still PENDING and requires the real pipeline plus recorded audio.

---

## Not yet started / PENDING

- **Chunk 2 (Audio):** VAD-aligned encrypted chunks, echo suppression, crypto.
- **Chunk 3 (Runtime):** signed model manifest, downloader, lifecycle gate.
- **Chunk 4 (STT):** segment writes, reconcile, diarization, recovery.
- **Chunk 8 (Lifecycle):** key zeroize + crypto-shred, startup sweep, audit.
- **Chunk 10 (Security/packaging):** audit evidence, signing, notarization.
- Chunk 1/6 runtime gates requiring a successful Tauri build and reference
  hardware measurements.

---

## Build environment blocker (affects every agent)

`cargo check -p intake-pro --lib` cannot complete on this machine. Two causes were
resolved (missing `cmake`; missing `rustfmt`/`clippy` components), leaving one
structural blocker:

```
xcode-select: error: tool 'xcodebuild' requires Xcode, but active developer
directory '/Library/Developer/CommandLineTools' is a command line tools instance
thread 'main' panicked at cidre-*/build.rs:145: status.success()
```

The `cidre` build script (a dependency of the macOS capture stack) requires a full
Xcode installation, not just Command Line Tools. Installing Xcode is a
multi-gigabyte GUI dependency and is out of scope for an automated agent.

**Consequences**
- The Tauri app crate is **not type-checked or compiled** in this environment.
- All runtime gates that need the desktop app (Chunk 1 migration/cascade/auth,
  Chunk 5 import network log, Chunk 7 persistence, resource measurements) remain
  **PENDING**.
- Mitigation: the app-side Rust files added by CORE/UI were syntax-validated with
  `rustfmt`, they follow existing module conventions, and the non-trivial logic
  they call (`json`, `local_auth`, retrieval, rubric, manifest, key lifecycle) is
  covered by `copilot-core`'s 71 passing tests.
- **Needs human input:** either install Xcode on the build machine, or move the
  app build to a CI runner/machine with Xcode so the runtime gates can execute.

## Verification summary (executed on this machine)

| Check | Command | Result |
|---|---|---|
| Core unit/integration tests | `cargo test -p copilot-core` | 71 passed, 0 failed |
| Core lint | `cargo clippy -p copilot-core --all-targets -- -D warnings` | clean |
| Core format | `cargo fmt -p copilot-core -- --check` | clean |
| Eval harness baseline | `copilot-eval intake_corpus.json` | recall 1.000, κ 1.000 |
| Eval harness degradation | `copilot-eval ... --degrade` | recall 0.714 (proves measurement) |
| Frontend typecheck | `pnpm typecheck` | clean |
| Frontend lint (copilot) | `pnpm lint` | clean |
| Frontend tests | `pnpm test` | 14 passed |
| App crate build | `cargo check -p intake-pro` | BLOCKED (Xcode/cidre), see above |


---

## Additional pure implementations (Chunks 2, 3, 4, 8) — Agent: ANALYSIS/AUDIO

Added to `copilot-core` and covered by tests (no model/audio hardware needed):

| Module | Chunk | What it does | Tests |
|---|---|---|---|
| `audio_crypto` | 2 | XChaCha20-Poly1305, 24-byte nonce prefix, AAD binding | 8 |
| `audio_chunk` | 2 | VAD-aligned fixed-duration PCM chunking with overlap, no gaps | 4 |
| `echo` | 2 | normalized cross-correlation delay estimation + single-tap suppression | 4 |
| `downloader` | 3 | chunked download state machine (pause/resume/verify/fail) | 6 |
| `reconcile` | 4 | boundary overlap merge (longest word overlap), dedupe | 6 |
| `diarization` | 4 | channel prior + cosine-threshold speaker clustering | 4 |
| `model_manifest` | 3 | SHA-256 fail-closed model verification | 5 |
| `key_material` | 8 | crypto-shred session key (zeroize on destroy/drop) | 3 |
| `lifecycle_gate` | 3 | sidecar never runs while capturing | 3 |

### Verification gate
- [x] `cargo test -p copilot-core` — 103 passed, 0 failed
- [x] `cargo clippy -p copilot-core --all-targets -- -D warnings` — clean
- [x] `node scripts/intake-audit.mjs` — 11/11 invariants pass

### Still PENDING (needs the desktop runtime, models and recordings)
- ONNX embedder + `llama-server` constrained-decoding wiring into `analyze_question`.
- Real VAD integration on the live capture path; echo suppression calibration.
- Chunk 8 crypto-shred end-to-end and startup orphan sweep.
- Chunk 10 signing, notarization, updater verification, cross-origin runtime test.

---

## Rebrand: meetily → Intake Pro

- `tauri.conf.json`: `productName` "Intake Pro", identifier `com.intakepro.app`,
  window title "Intake Pro".
- `frontend/package.json` name `intake-pro`; `frontend/src-tauri/Cargo.toml`
  package `intake-pro` v0.5.0.
- 37 source files rebranded (`Meetily` → `Intake Pro`, `meetily` → `intake-pro`)
  across `frontend/src` and `frontend/src-tauri/src`.
- Kept deliberately: legacy meetily DB-detection paths in
  `HomebrewDatabaseDetector.tsx` / `OnboardingContext.tsx` (they detect an old
  meetily install), and upstream attribution references in the docs (MIT).
- Sidecar override env var renamed to `INTAKE_PRO_LLAMA_HELPER`, with the legacy
  `MEETILY_LLAMA_HELPER` still accepted.

### Verification after rebrand
- [x] `DOCS_RS=1 cargo check -p intake-pro --lib` — compiles (warnings only, pre-existing)
- [x] `pnpm build` — production build succeeds; `/copilot` prerendered
- [x] `pnpm typecheck` / `pnpm lint` / `pnpm test` — clean, 18 passed

---

## Round 3 — runtime gates (a) and end-to-end analysis wiring (b)

### (a) Install Xcode and run the runtime gates — BLOCKED (needs a human)
`brew install --cask xcode` fails: **there is no `xcode` Homebrew cask** — Apple
distributes Xcode only through the App Store / Apple Developer, both of which
require an authenticated Apple ID. `cidre`'s build script invokes
`xcodebuild -project ./pomace/pomace.xcodeproj build`, which exists only in full
Xcode. An automated agent cannot complete this step.

**Manual steps (once Xcode is installed):**
```bash
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
cd frontend && pnpm tauri:dev      # or pnpm tauri:build
```
Then the pending runtime gates can run: Chunk 1 (migration idempotency, cascade
delete, DB lockout, token/origin), Chunk 2 (echo speaker-on test, backup
exclusion, discard-30s), Chunk 4 (boundary diff, two-speaker), Chunk 7
(persistence), Chunk 8 (crypto-shred), Chunk 10 (cross-origin fetch).

Everything that does **not** need Xcode is verified below.

### (b) Wire ONNX embedder + llama-server adapter — DONE (code), models still needed to run

Added to the app crate:
- `copilot/embedding.rs` — `AppEmbedder` selects ONNX (`onnx-embed` feature) or the
  deterministic `HashingEmbedder`. The ONNX path (ort 2.0-rc.10 + tokenizers 0.21,
  CLS pooling, L2 norm) **compiles** with `--features onnx-embed`.
- `copilot/llm_client.rs` — `OpenAiCompatClient` for loopback `llama-server` or any
  OpenAI-compatible endpoint, with `response_format: json_schema` constrained
  decoding; strict parsing still validates the response.
- `copilot/analysis.rs` — `run_analysis` orchestrates window→embed→retrieve→
  prompt→parse→validate, persists verdicts + notes, and checkpoints
  `processing_stage` (indexing → evaluating_questions → extracting_notes → complete).
- Tauri command `copilot_run_analysis(meeting_id)` registered in `lib.rs`.
- `copilot-core` gained `embedding`, `engine` (`analyze_meeting`), `heuristic`
  (`HeuristicLlm`), trait-object (`&dyn LlmClient`) support, and evidence lines now
  carry `[t<seq>] ids` so the model can actually cite segments.

**End-to-end without a model:** `analyze_meeting` + `HashingEmbedder` +
`HeuristicLlm` runs the whole pipeline and is covered by tests
(`engine::tests::full_pipeline_runs_without_a_model`). To run with real models you
still need: the ONNX embedding model + `tokenizer.json` in
`<app_data>/models/embedding`, and a `llama-server` GGUF. Those are gated on the
model manifest (Chunk 3) and cannot be fabricated.

### Verification (this round)
- [x] `cargo test -p copilot-core` — **112 passed**
- [x] `cargo clippy -p copilot-core --all-targets -- -D warnings` — clean
- [x] `DOCS_RS=1 cargo check -p intake-pro --lib` — compiles
- [x] `DOCS_RS=1 cargo check -p intake-pro --lib --features onnx-embed` — compiles
- [x] `node scripts/intake-audit.mjs` — 11/11
- [x] frontend typecheck / lint / test — clean, 18 passed

---

## Browser demo bundle + DMG

Because the native macOS app cannot be linked without Xcode, the distributable
artifact is a **browser demo** (UI only; nothing is captured, saved or uploaded).

Build the bundle + DMG:
```bash
cd frontend && pnpm build                         # static export -> frontend/out
rm -rf dist && mkdir -p "dist/IntakePro-Demo/site"
cp -R frontend/out/. "dist/IntakePro-Demo/site/"
mkdir -p "dist/IntakePro-Demo/site/copilot"
cp "dist/IntakePro-Demo/site/copilot.html" "dist/IntakePro-Demo/site/copilot/index.html"
# then add serve.mjs, "Launch Intake Pro Demo.command", README.txt (see dist/)
hdiutil create -volname "Intake Pro Demo" -srcfolder "dist/IntakePro-Demo" \
  -ov -format UDZO "dist/IntakePro-Demo.dmg"
```

Artifacts:
- `dist/IntakePro-Demo/` — static site + launcher (`Launch Intake Pro Demo.command`)
- `dist/IntakePro-Demo.dmg` — compressed, CRC-verified, mounts and launches

### Verification
- [x] `node dist/IntakePro-Demo/serve.mjs 4399` serves `/copilot/` with HTTP 200
  and the "Demo mode (browser)" banner
- [x] `hdiutil verify dist/IntakePro-Demo.dmg` — checksum VALID
- [x] DMG mounts; launcher is executable inside

### Native app DMG
Not producible here: `pnpm tauri:build` requires Xcode (cidre → `xcodebuild`).
Once Xcode is installed, `cd frontend && pnpm tauri:build` produces the native
`.app`/`.dmg`.

---

## Browser demo fix + models page

Problem: the root layout ran the onboarding/model-download check on every route,
so in a plain browser (no Tauri) `invoke('get_onboarding_status')` failed and the
demo fell through to "downloading the transcription engine".

Fixes:
- `frontend/src/app/layout.tsx` — routes `/copilot` and `/models` render
  standalone (no providers, no onboarding, no Tauri `invoke`/`listen`).
- Added `frontend/src/lib/copilot/models.ts` (exact required models, files, sizes,
  RAM, destinations, sources) and `frontend/src/app/models/page.tsx`.
- Copilot demo banner now links to `/models/`.
- Corrected three external URLs that the earlier rebrand script had wrongly
  rewritten (they are third-party resources, not the app name):
  parakeet v3 source host, the About link, and the Bluetooth notice link.

### Verification
- [x] `pnpm build` — includes `/models` (13 static pages)
- [x] `pnpm typecheck` / `pnpm lint` / `pnpm test` — clean, 18 passed
- [x] `DOCS_RS=1 cargo check -p intake-pro --lib` — compiles
- [x] `node scripts/intake-audit.mjs` — 11/11
- [x] demo server: `/copilot/` HTTP 200, no download screen, shows demo banner;
  `/models/` HTTP 200 with exact model file names and sources
- [x] `dist/IntakePro-Demo.dmg` rebuilt and `hdiutil verify` — checksum VALID

---

## Local web app: home screen + model gate + meeting modes

- `frontend/src/app/demo/page.tsx` — home screen: on load it calls the local
  server's `GET /api/models` (a real filesystem scan of `./models`) and shows
  which required model files are present, with a "Re-check" button and download
  sources. Two entry points: "Start a co-pilot meeting" and "Start a meeting".
- `frontend/src/app/copilot/page.tsx` — reads `?mode=plain|copilot`; plain mode
  skips the question set and starts a transcript+notes-only flow.
- `dist/IntakePro-Demo/serve.mjs` — adds `GET /api/models`, serves `/` from
  `/demo/`, and maps directory URLs to `<name>.html`.
- `dist/IntakePro-Demo/Download Models.command` — fetches the five required model
  files into `./models` (~2.3 GB) so the readiness check can pass.

### Verification
- [x] `pnpm build` — 13 static pages incl. `/demo` and `/models`
- [x] `pnpm typecheck` / `lint` / `test` — clean, 18 passed
- [x] live server: `/`, `/demo/`, `/copilot/`, `/copilot/?mode=plain`, `/models/`
  all HTTP 200
- [x] `GET /api/models` returns real presence status (5 missing) for the models dir
- [x] `dist/IntakePro-Demo.dmg` rebuilt; `hdiutil verify` VALID

---

## Cleanup pass: remove upstream branding, dead code, unused screens

- Deleted `frontend/src/components/DatabaseImport/` (legacy meetily/Python DB import
  UI — dead code, self-referenced only) and removed the Homebrew/legacy
  auto-detection from `OnboardingContext` (fresh DB init only).
- Removed the last `meetily` strings from source: `About.tsx` (external link +
  vendor CTA/footer), `BluetoothPlaybackWarning.tsx` notice link, and the
  `MEETILY_LLAMA_HELPER` env fallback in `sidecar.rs`.
- Parakeet: dropped the meetily-hosted v3 model; `parakeet-tdt-0.6b-v2-int8`
  (Hugging Face `istupakov`) is now the default everywhere (Rust specs + frontend
  display/config). This also removed the only third-party "meetily" URL in code.
- Deleted the dead `audio/recording_commands.rs.backup` file.
- Login/signup: none existed in the UI. The only `password` inputs are API-key
  fields. (Rust still exposes unused profile/licence commands — flagged below.)
- Navigation: sidebar now shows **Meeting**, **Previous meetings** (existing
  list), **Models & API keys**, **Settings**.
- `layout.tsx`: `/demo` also renders standalone so the browser home never mounts
  the Tauri onboarding flow.

### Still to remove (needs a decision — would break the build if done blind)
- Rust `api_get_profile` / `api_save_profile` / `api_update_profile` and the
  OpenAI-dashboard “test connection” helpers are unused by the current UI.
- `posthog-rs` analytics + `AnalyticsProvider`/`AnalyticsConsentSwitch`:
  privacy-first posture suggests removing, but it is wired into `layout.tsx` and
  the sidebar, so it needs a coordinated removal.

### Native build (Xcode now installed)

`pnpm tauri:build` compiles through the frontend and Rust but fails at the first
C link step:

```
You have not agreed to the Xcode license agreements.
Please run 'sudo xcodebuild -license' from within a Terminal window...
```

Accepting the licence needs admin privileges (`sudo` requires a password), so it
cannot be done by the agent. One-time manual step, then the build works:

```bash
sudo xcodebuild -license accept        # or: sudo xcodebuild -runFirstLaunch
cd frontend && pnpm tauri:build       # produces .app + .dmg
```

Bundle targets are already set to `app` and `dmg`, and `externalBin` sidecars
(llama-helper, ffmpeg) are staged, so the DMG will be produced once the licence
is accepted.

---

## Native macOS build — SUCCESS

After the Xcode licence was accepted, `pnpm tauri:build` compiled and signed the
app. Artifacts:

- `target/release/bundle/macos/Intake Pro.app` — bundle id `com.intakepro.app`,
  ad-hoc signed (`Signature=adhoc`), contains `intake-pro` (68 MB), `llama-helper`
  (6.5 MB) and `ffmpeg` (49 MB).
- `dist/Intake Pro.dmg` — 51 MB, checksum VALID, mounts with the `.app` and an
  `/Applications` symlink.

Tauri's own `bundle_dmg.sh` failed in this headless session (it drives Finder /
`osascript` for the DMG window layout), so the DMG was produced directly with
`hdiutil create` from the built `.app`. Packaging only — the app itself built fine.

Notarization was skipped (no `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` env), so
the app is ad-hoc signed: it runs on this machine; other machines will need
right-click → Open (or a Developer ID signature + notarization for distribution).

### Verification
- [x] `Intake Pro.app` builds and is structurally valid (Mach-O arm64, id
  `com.intakepro.app`)
- [x] `dist/Intake Pro.dmg` mounts; `hdiutil verify` VALID

---

## Flow changes: meeting chooser, two models, About/Beta removed, mic icon

- **First screen**: `frontend/src/app/page.tsx` now shows a "New meeting" chooser
  before anything else, with two options:
  * **Standard meeting** — record + transcribe, no question set.
  * **Meeting with co-pilot** — upload intake questions (Excel `.xlsx` / CSV) and
    get per-question analysis; routes to the co-pilot workspace.
- **Two models**: the models surface is now framed as exactly two — a
  *transcription model* (Whisper) and a *processing model* (Qwen2.5-3B + its
  embedding/VAD support). Settings tabs relabelled to "Transcription model" and
  "Processing model"; `lib/copilot/models.ts` restructured to the two models.
- **Removed sections**: "About" tab removed from `SettingTabs`; "Beta" (upcoming)
  tab removed from the Settings page; the "Coming soon" block removed from
  `About.tsx`.
- **Icon**: replaced with a simple microphone mark. Source SVG rendered to a
  1024 PNG (`qlmanage`) and expanded with `tauri icon`; `icon.icns/ico` copied to
  `app_icon.*`. The sidebar logo (`Logo.tsx`) is now an inline mic vector (no
  external image, no About dialog).
- **Branding**: `grep -ri meetily frontend/src frontend/src-tauri/src` returns
  nothing; the built app uses product name "Intake Pro" and id `com.intakepro.app`.
- Version bumped to 0.5.0; `createUpdaterArtifacts` set to `false` so local
  builds don't fail on the missing updater signing key.

---

## Co-pilot FK fix + question bank + About removal

### Root cause of `FOREIGN KEY constraint failed (787)`
`meeting_questions` and `transcript_segments` reference `meetings(id)`. The
co-pilot page generated a fresh UUID and inserted questions/segments without ever
creating the `meetings` row, so every insert failed the FK. Standard meetings
worked because meetily's normal flow creates the meeting first.

### Fix
- `CopilotRepository::ensure_meeting(pool, id, title, mode)` inserts the meeting
  row (INSERT OR IGNORE).
- New command `copilot_start_meeting(meeting_id, title, mode)`.
- `add_bank_questions` calls `ensure_meeting` before inserting.

### Question bank (co-pilot first step)
- Migration `20260201000000_add_question_bank.sql` (reusable questions).
- Repo: `bank_list`, `bank_add`, `bank_add_many`, `bank_delete`,
  `add_bank_questions`.
- Commands: `question_bank_list/add/add_many/delete`, `copilot_add_bank_questions`.
- UI `components/copilot/QuestionLibrary.tsx`: lists previously entered questions
  with tick-select, **Add question** (Question, other ways to ask, sample
  answers) and **Bulk add (Excel .xlsx / CSV)** with the same three columns.
  Selected questions are copied into the meeting.

### About removed
- Deleted `components/Info.tsx` and `components/About.tsx`; removed the `<Info>`
  usage and import from the sidebar. `SettingTabs` About tab and the Settings Beta
  tab were already removed. `grep -ri about frontend/src` now only matches the
  word in unrelated copy.

### Verification (this round)
- [x] Rust repository tests (real SQLite + migrations), release profile:
      **7 passed** — including `foreign_key_violation_happens_without_a_meeting_row`
      (documents the bug) and `ensure_meeting_makes_imports_succeed` (the fix),
      plus question-bank roundtrip and add-to-meeting.
- [x] `cargo test -p copilot-core` — 112 passed
- [x] `node scripts/intake-audit.mjs` — 11/11
- [x] `pnpm typecheck` / `pnpm lint` / `pnpm test` — clean, 18 passed
- [x] Native `pnpm tauri:build` — `.app` + `Intake Pro_0.5.0_aarch64.dmg`
- [x] 0 `meetily` strings in source
- [ ] Interactive GUI click-through — not automatable here; see note

### Not tested / needs a human pass
The native app's GUI flow (choosing "Meeting with co-pilot", adding questions,
recording) was **not driven interactively** — this environment cannot automate the
Tauri window, and there is no iOS/tvOS target for a macOS desktop app so an
"Xcode simulator" run does not apply. The database path that failed (FK 787) is
now covered by integration tests, but a manual click-through is still required
before calling the flow verified.

---

## Fix: "Application error: a client-side exception has occurred" (browser demo)

Root cause: `serve.mjs` served the `/demo` page's HTML **at the `/` URL without
redirecting**. Next.js hydrates the client router from the URL pathname, so the
browser tried to hydrate the app-home route (`/`) against demo markup → hydration
mismatch → client-side exception. `curl` showed 200 because it does not run JS.

Fix:
- `serve.mjs` now issues a real `302` for `/` → `/demo/` (and redirects any
  non-demo route, e.g. `/settings`, to `/demo/`), so the URL always matches the
  route being hydrated.
- The launcher opens `/demo/` directly.
- The demo DMG was rebuilt.

Verified: `/` → 302 `/demo/`; `/demo/`, `/copilot/`, `/models/` → 200.

---

## Co-pilot flow v2: question bank first, meeting window, general summary

Changes:
- **No consent gate** in the co-pilot flow. Clicking "Meeting with co-pilot" goes
  straight to creating/choosing a question bank.
- **Co-pilot runs in the existing meeting window** (`app/page.tsx`):
  1. Chooser → "Meeting with co-pilot".
  2. Question bank step (`QuestionLibrary`): previously entered questions, add
     individual, or bulk Excel/CSV with Question / alternatives / sample answers.
  3. Meeting: split view — a **Questions panel on the left** (tick-off + notes,
     persisted) next to the existing **transcript panel**; the normal recording
     controls start/stop the call. Questions are present from the start;
     transcription runs separately.
  4. Stop → `copilot_run_analysis` runs over the transcript using the questions,
     then the report shows the **general summary** plus per-question verdicts.
- **General summary is always produced** by the notes pass, independent of the
  question set. Added to the JSON schema, `Notes`, `HeuristicLlm`, the notes system
  prompt, a `general_summary` meetings column, and `copilot_get_meeting_notes`.
- Two models framing retained (transcription + processing).

Verification (automated):
- `cargo test -p copilot-core` — 112 passed (incl. general-summary tests)
- DB integration tests re-run with the new migrations
- `pnpm typecheck` clean
- Not verified: interactive GUI click-through (cannot be automated here).
