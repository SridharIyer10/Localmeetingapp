# Intake Co-Pilot — Development Plan

**Source specification:** `intake-copilot-build-spec.md`
**Base project:** [Zackriya-Solutions/meetily](https://github.com/Zackriya-Solutions/meetily) (MIT, confirmed)
**Plan date:** 2026-10-19
**Status:** Draft for approval — Wave 0 not started

---

## 1. Summary

Intake Co-Pilot is a fork-and-extend desktop app for recording, transcribing, and evaluating recruiter intake calls. It reuses meetily's Tauri (Rust) shell, dual-channel audio capture, SQLite persistence, whisper.cpp integration, and Next.js frontend. The net-new work is:

1. A strict, versioned data model for questions, transcript segments, and analysis settings.
2. VAD-aligned encrypted audio chunks with crypto-shredding disposal.
3. Manifest-verified model management and a lifecycle-controlled `llama-server`.
4. A post-call analysis pipeline based on ONNX embeddings, hybrid retrieval, and constrained LLM output with evidence citations.
5. Question ingest, a static teleprompter, a report/correction UI, and an evaluation harness.
6. Local-service hardening, signing, notarization, and packaging.

The plan follows the specification's **10 chunks**, **6 execution waves**, **mandatory reuse checks**, and **blocking verification gates**. A chunk is not complete until its gate is evidenced line by line.

---

## 2. Starting point and gap analysis

The local repository is an empty Git repository with no commits. The first action is to import the upstream project at a pinned commit, preserving its MIT licence and history.

Early inspection of the upstream tree (536 files) indicates that most of the required substrate already exists. These are **candidate reuse targets, not confirmed facts**; the RECON chunk must cite file paths and line ranges and produce `docs/REUSE_MAP.md`.

| Capability | Probable upstream location | Likely gap | Planned action |
|---|---|---|---|
| Dual-channel capture | `frontend/src-tauri/src/audio/capture/`, `audio_v2/` | VAD-aligned chunking, encryption, echo suppression, discard-last-30s | Extend |
| VAD | `frontend/src-tauri/src/audio/vad.rs` | Silero ONNX, chunk-boundary policy | Extend or replace adapter |
| Whisper integration | `audio/transcription/worker.rs`, `whisper_engine/` | Background priority caps, overlap reconciliation, recovery sweep | Extend |
| Database | `frontend/src-tauri/src/database/`, `migrations/` | New copilot tables, SQLCipher, per-connection pragmas | Extend migrations |
| Sidecar/model management | `summary/summary_engine/sidecar.rs`, `model_manager.rs` | Manifest signing, lifecycle gating, hardware probe | Extend/replace |
| Frontend shell | `frontend/src/app/`, `frontend/src/components/` | Question ingest, teleprompter, report view | Extend |
| Python backend | `backend/app/{main,db,transcript_processor}.py` | Whether it is load-bearing is unknown | **Decide in Chunk 0** |
| Packaging/CI | `.github/workflows/build-*.yml`, `release.yml`, entitlements | Signing, notarization, updater pinning, third-party licences | Extend |
| Permissions | `audio/permissions.rs`, `check_screen_permission.swift` | macOS Screen Recording request/denied/re-prompt flow | Extend |

**Architectural caveat:** the Python FastAPI backend may duplicate responsibilities already implemented in Rust (database repositories, whisper engine, summary sidecar). If RECON shows it is a thin wrapper, bypass it. That removes a network listener, a dependency surface, and the pandas-free import requirement becomes trivial. This decision determines the embeddings implementation language and must be explicit in `REUSE_MAP.md`.

---

## 3. Scope boundaries

### In scope
- macOS 13+ and Windows 10 2004+.
- English only.
- Single-user desktop app.
- Post-call analysis only.
- Manual question tick-off as ground truth.
- Embedded local models by default; optional external endpoint.
- Markdown and plain-text export.

### Out of scope (enforced by review and greps)
- Live question detection, matching, or status changes during a call.
- Streaming LLM inference while recording.
- Meeting bots, cloud transcription, or audio upload.
- ATS/CRM write-back, third-party local HTTP API, structured JSON export.
- Multi-user accounts.
- `torch` or `sentence-transformers`.
- Vector databases.

### Global invariants
These are release blockers, not guidelines:

1. `llama-server` never runs while audio is captured.
2. No audio file exists outside the designated cache directory.
3. No API key exists outside the OS secret store.
4. No local service accepts a request without the session token.
5. No model loads without a manifest digest match.
6. No live question detection exists anywhere.
7. Every automated verdict is user-correctable.
8. Every automated verdict cites transcript evidence.

Add these to `AGENTS.md` and a CI invariant check.

---

## 4. Delivery strategy

### 4.1 Execution waves

| Wave | Work | Gate to leave the wave |
|---|---|---|
| 0 | RECON | `REUSE_MAP.md` reviewed and approved by a human |
| 1 | CORE | Schema, migration, secret-store, and local-auth gates green |
| 2 | AUDIO, RUNTIME, UI ingest/teleprompter, ANALYSIS against fixtures | Each chunk's gate green, files do not collide |
| 3 | STT, EVAL harness scaffold | Transcription and boundary reconciliation gates green |
| 4 | AUDIO lifecycle, UI report, ANALYSIS wiring | End-to-end session produces a correct, citable report |
| 5 | SEC audit, packaging, EVAL full run | Signed builds, audit checklist, honest baseline metrics |

Agents/engineers in the same wave must not edit the same files. When they must, the later chunk waits.

### 4.2 Milestones

| Milestone | Target | Exit evidence |
|---|---|---|
| M0 Recon approved | End of week 1 | `REUSE_MAP.md` with cited paths, backend decision, deviations |
| M1 Foundation | End of week 3 | Migrations idempotent; SQLCipher; token-gated loopback service |
| M2 Capture + runtime | End of week 5 | Encrypted chunks; speaker-on echo test; model manifest; no sidecar during recording |
| M3 STT + ingest | End of week 7 | 30-min recording transcribed within 60s; import and teleprompter work |
| M4 Analysis on fixtures | End of week 8 | Evidence citations; injection resistance; schema-valid output; resume works |
| M5 End-to-end report | End of week 10 | Live call → report; clickable evidence; corrections persist; export |
| M6 Lifecycle + eval baseline | End of week 11 | Crypto-shred verified; metrics report with real numbers |
| M7 Release candidate | End of week 13 | Signed/notarized builds; audit checklist; dependency audit triaged |

Calendar assumes a **three-person build team** (Rust/audio, full-stack/UI, ML/analysis) plus part-time QA and security review. A single engineer should plan for 18–22 weeks. Estimates exclude Legal review and corpus collection lead time.

---

## 5. Work breakdown

Estimates are person-days and include implementation, tests, and gate evidence. They are planning ranges, not commitments.

| Chunk | Agent | Depends on | Estimate | Primary deliverables | Blocking gate highlights |
|---|---|---|---|---|---|
| 0 Recon | RECON | — | 4–6 | `docs/REUSE_MAP.md` | Every claim cites path + lines; backend question answered |
| 1 Foundation | CORE | 0 | 6–8 | Migrations, schema, secret store, hardened bootstrap | Double-run migrations; cascade deletes; locked-DB test; token/CORS test |
| 2 Audio | AUDIO | 1 | 12–15 | Chunker, Silero VAD, XChaCha20, platform extensions, privacy controls | No gaps/overlap loss; no PCM in files/memory; backup exclusion; echo test |
| 3 Runtime | RUNTIME | 1 | 8–10 | Signed manifest, downloader, supervisor, hardware probe | Tampered model rejected; no sidecar during recording; key absent from `ps` |
| 4 STT | STT | 2, 3 | 10–14 | Worker, reconciler, diarization, recovery | Boundary diff clean; distinct speakers; crash recovery; no frame drops |
| 5 Ingest + teleprompter | UI | 1 | 8–10 | CSV/XLSX import, validation, teleprompter, consent gate, settings | Comma-safe import; size/row caps; no upload endpoint; consent persisted |
| 6 Analysis | ANALYSIS | 1, 3 | 12–16 | Indexer, hybrid retrieval, evaluator, notes, checkpointing, overrides | Evidence validity; injection resistance; 100/100 schema-valid; resume; RSS |
| 7 Report + export | UI | 6 | 6–8 | Report view, corrections, Markdown/text renderers | Evidence scroll; correction persistence; UTF-8 round-trip; no unsafe HTML |
| 8 Lifecycle | AUDIO | 4, 6 | 5–7 | Disposal flow, key lifecycle, startup sweep, audit log, copy | Key and chunks gone after confirm; decline path works; orphan sweep; full-disk search |
| 9 Evaluation | EVAL | 6 | 8–12 | Corpus, runner, metrics, gate script | Reproducible; k=1 lowers recall; real baseline reported |
| 10 Security + packaging | SEC | all | 10–14 | Audit evidence, signed/notarized builds, updater, licence file | Cross-origin fetch blocked; digests enforced; clean-machine launch; tampered update rejected |

**Total:** roughly 89–120 person-days of build effort. Add 15–20% contingency for platform-specific surprises (Windows loopback, SQLCipher integration, Core ML).

### 5.1 Long-lead items (start immediately, run in parallel)

- **Legal:** consent language, retention policy, two-party-consent jurisdictions, GDPR, India DPDP Act. Blocks deployment, not development.
- **Evaluation corpus:** 20+ calls or realistic synthetic transcripts with labels and consent. This is the most likely schedule slip. Start a synthetic fixture set in week 1 and add real sessions as consent allows.
- **Reference hardware:** one 8 GB macOS machine and one 8 GB Windows machine, available for every wave gate.
- **Model/licence review:** Qwen2.5-3B is Apache-2.0. Flag any Llama-3.x substitution for commercial review.
- **Diarization model selection:** the spec requires speaker clustering but does not name a model. Choose an ONNX speaker-embedding model compatible with the no-torch rule before Chunk 4.

---

## 6. Detailed phase plan

### Wave 0 — Recon (Chunk 0)

**Tasks**
1. Import upstream at a pinned commit; keep `LICENSE.md`; record the commit hash.
2. Read the audio capture, transcription, persistence, backend, frontend, model-management, and security code.
3. Answer the backend question unambiguously: load-bearing or bypassable.
4. Map every chunk to existing files and justified new files.
5. List deviations between the spec and reality, especially:
   - existing API-key columns in migrations (`openrouter`, `gemini`, `openai`, `pro_license`);
   - existing live transcription/summary features that could resemble live detection;
   - existing sidecar startup paths that may run during capture;
   - whether SQLx migrations and SQLCipher are compatible with the current build.
6. Produce `docs/REUSE_MAP.md` and stop.

**Gate:** human review. Do not begin Wave 1 before approval.

### Wave 1 — Foundation (Chunk 1)

**Tasks**
1. Extend the existing migration mechanism; do not add a second runner.
2. Add the copilot schema from §3.4 as a versioned migration.
3. Enforce `foreign_keys=ON` and `journal_mode=WAL` on every connection.
4. Add SQLCipher with the key in Keychain/DPAPI; provide a no-secret-in-config path.
5. Harden the local service: loopback-only, ephemeral port, deny CORS, validate `Origin`/`Host`, per-session bearer token injected by the shell.
6. Define the secret-store abstraction and remove API-key columns from new code paths.
7. Add migration and service integration tests.

**Gate:** all eight checks in Chunk 1, with pasted command output.

### Wave 2 — Capture, runtime, ingest UI, analysis against fixtures

**Chunk 2 (Audio)**
- Confirm and extend the existing capture layer; do not rewrite it.
- Add VAD-aligned ~30 s chunks with 2 s overlap, monotonic timestamps, channel tags.
- Add per-session XChaCha20-Poly1305 encryption and `zeroize`.
- Add backup/sync exclusion, cache-directory enforcement, core-dump disablement.
- Implement cross-correlation echo suppression; validate with the speaker-on test.
- Add Bluetooth HFP detection, pause/resume, discard-last-30s.
- Gate includes peak RSS ≤600 MB over a 10-minute capture.

**Chunk 3 (Runtime)**
- Ship a signed model manifest with digests, sizes, licences, minimum RAM.
- Build a resumable HTTPS downloader with progress/cancel/disk checks.
- Add a sidecar supervisor that starts `llama-server` only for analysis, binds loopback, uses a random port, and passes the API key via environment.
- Add hardware probing and tier recommendations.
- Confirm Core ML encoder use on Apple Silicon; cap threads and priority.

**Chunk 5 (UI ingest + teleprompter)**
- Build manual entry and client-side SheetJS CSV/XLSX parsing.
- Enforce 2 MB/500-row caps, header validation, per-row errors, sample template.
- Store multi-value fields as JSON; accept one variant per row or `|`.
- Add the static teleprompter, manual tick-off, notes, timer, un-ticked count.
- Add consent gate and persistent recording indicator.
- Add settings for provider mode, models, devices, Keychain-backed keys, and a loud cloud-mode confirmation.

**Chunk 6 (Analysis, fixtures first)**
- Window transcript into ~3–4 turn windows with 1-turn overlap.
- Embed with ONNX `bge-small-en-v1.5` or `multi-qa-MiniLM-L6-cos-v1`; brute-force cosine.
- Blend cosine with BM25.
- Retrieve top-k=5 per question phrasing.
- Call the LLM once per question over evidence only, with JSON-schema/GBNF constrained decoding.
- Put the rubric in the system message; label transcript as data; escape delimiters; cap tokens.
- Validate rating/status orthogonality post-hoc.
- Add map-reduce notes extraction with dedupe.
- Checkpoint stages and support resume and re-run.
- Add manual override.

**Gate:** every chunk's own gate must pass before Wave 3.

### Wave 3 — Transcription and evaluation scaffold

**Chunk 4 (STT)**
- Extend the existing whisper invocation; do not reimplement bindings.
- Decrypt chunk → transcribe → persist segments → mark chunk done, at low priority.
- Reconcile overlaps and dedupe at boundaries.
- Assign speakers using channel as prior plus embedding clustering within loopback.
- Queue under backpressure; never drop audio or block capture.
- Flush tails on Stop; set `transcription_completed_at`.
- Add startup recovery for untranscribed chunks.

**Chunk 9 (EVAL scaffold)**
- Define fixture format and label schema.
- Seed synthetic fixtures immediately; add real consented calls as available.
- Build the runner, metrics (detection P/R, status accuracy, Cohen's κ, latency, RSS), and CI report.
- Establish a baseline. Report real numbers even if below suggested thresholds.

**Gate:** boundary diff and two-speaker tests are blocking.

### Wave 4 — End-to-end integration

- Wire the analysis engine to real transcript rows.
- Build the report view with clickable evidence, corrections, action points, next steps, actions discussed.
- Add Markdown and plain-text export with UUID filenames.
- Append full speaker-labelled transcript.
- Verify the complete timeline: record → stop → finalise → analyse → review → dispose.
- Measure latency and RSS on the 8 GB reference machines.

**Gate:** clickable evidence, correction persistence, UTF-8 round-trip, no unsafe HTML, path traversal impossible, zero-question export valid.

### Wave 5 — Lifecycle and release

**Chunk 8 (Lifecycle)**
- Prompt for disposal only after the report is shown.
- On confirm: zeroize the key, unlink chunks, write `recording_destroyed_at`.
- On decline: retain encrypted, expose later deletion.
- Startup sweep for orphaned chunks.
- Destruction on meeting delete, uninstall path, disk-pressure threshold.
- Audit each destruction.
- Use honest UI copy: the transcript is retained.

**Chunk 10 (Security + packaging)**
- Independently verify every audit line; do not trust earlier chunk reports.
- Test a real cross-origin browser fetch against both local services.
- Run `cargo audit`, `npm audit`, `pip-audit`; triage findings.
- Sign and notarize macOS; sign Windows; verify on clean machines.
- Pin llama.cpp/whisper.cpp commits; record build hashes.
- Ship third-party licence file.
- Verify tampered update rejection.

**Gate:** every audit line has evidence; any failure blocks release.

---

## 7. Critical path and schedule

```
0 Recon → 1 Core → 2 Audio → 4 STT → 6 Analysis → 8 Lifecycle → 10 Security/Release
                    └→ 3 Runtime ──┘        └→ 7 Report ──┘
1 Core → 5 UI ingest ─────────────────────────┘
6 Analysis → 9 Eval ─────────────────────────────────────────→ release gate
```

The longest chain is **0 → 1 → 2 → 4 → 6 → 8 → 10**, roughly 57–76 person-days. With three parallel tracks and gate reviews, target **13 weeks** to release candidate. The evaluation corpus and Legal review are the two non-engineering long poles.

---

## 8. Engineering standards

### Repository setup
1. Import upstream at a pinned commit as the initial commit.
2. Keep `LICENSE.md`; add a `NOTICE` describing modifications.
3. Add `AGENTS.md` with build/test commands, invariants, and the chunk report template.
4. Create `docs/` for the spec, reuse map, and this plan.
5. Protect `main`; do all work on chunk branches named `chunk/<n>-<slug>`.

### Required per-chunk report
Use the specification's template verbatim. A PR is not reviewable without:
- reuse decision with paths;
- new-dependency justification and resource impact;
- PASS/FAIL gate evidence;
- measurements;
- deviations;
- blocked/human-input items.

### CI on every PR
- Rust: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`.
- Frontend: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`.
- Python (if retained): `ruff`, `pytest`, `pip-audit`.
- Audits: `cargo audit`, `npm audit`.
- Invariant greps:
  - no `dangerouslySetInnerHTML`;
  - no live question-matching/similarity code in the frontend;
  - no API-key reads from DB/config paths;
  - no `torch` or `sentence-transformers`;
  - no audio path outside the cache module.
- Migration idempotency test on a fresh and legacy database.

### Definition of done for a chunk
1. All gate boxes pass with pasted evidence.
2. Unit/integration tests added for new behaviour.
3. No invariant violated.
4. Resource measurements recorded on reference hardware.
5. Chunk report completed.
6. Human reviewer signs off for Chunk 0 and Chunk 10; peer review for the rest.

---

## 9. Test and measurement strategy

| Layer | What | When |
|---|---|---|
| Unit | Migration, crypto, chunk boundaries, parsing, retrieval scoring, rubric enforcement | Every PR |
| Integration | Record → chunk → transcribe → analyse → report → dispose | Wave 4 onward |
| Platform | macOS 13+, Windows 10 2004+ capture and permissions | Every wave gate |
| Audio | 10/30/60-minute runs; speaker-on echo; Bluetooth HFP; discard-last-30s | Chunks 2, 4 |
| Analysis | Fixture corpus, injection fixture, 100-run schema validity, k=1 degradation | Chunks 6, 9 |
| Security | Cross-origin fetch, token bypass, digest tamper, DB lockout, full-disk magic-byte scan | Chunks 1, 3, 8, 10 |
| Resource | Peak RSS and wall time at record/analyse/idle against §4.5 | Every wave gate |
| Release | Clean-machine Gatekeeper/SmartScreen, notarization staple, tampered update | Chunk 10 |

Measure, do not estimate: peak RSS, frame drops, transcription lag, analysis latency, and WER for `small.en` vs `base.en`.

---

## 10. Risk register

| Risk | Signal | Response | Owner |
|---|---|---|---|
| Python backend is load-bearing in ways not visible in the tree | RECON cannot cleanly bypass it | Keep it, harden it, use Python ONNX; add auth and CORS tests | RECON/CORE |
| SQLCipher incompatible with current SQLx setup | Build or migration failure | Switch to bundled SQLCipher build or a dedicated encrypted store; do not weaken to plaintext | CORE |
| Windows per-process loopback unavailable | API returns unsupported | Session loopback with explicit warning; document capture scope | AUDIO |
| Echo suppression insufficient | Speaker-on test fails | Require headphones; state plainly in UI; do not ship wrong attribution | AUDIO |
| `small.en` too slow on low-end Windows | STT falls behind realtime | Hardware probe falls back to `base.en`; document trade-off | STT |
| Qwen2.5-3B too weak for rubric grading | Chunk 9 κ < 0.5 | Escalate before more UI; consider larger model or advisory ratings | ANALYSIS |
| 8 GB envelope missed | Analysis RSS > 3.75 GB | Reposition embedded LLM as 16 GB feature; external endpoint is 8 GB path | RUNTIME |
| Diarization model unspecified | Multiple remote speakers collapse | Select ONNX speaker embeddings early; if unavailable, ship channel labels only with honest UI | STT |
| Evaluation corpus delayed | No real labels | Start synthetic fixtures in week 1; auto-ingest `user_ticked`; report baseline honestly | EVAL |
| Constrained decoding unsupported in pinned llama.cpp | Parse failures or retries | Pin a version with JSON-schema/GBNF support; test 100 runs before UI work | ANALYSIS |
| Legal review delays launch | Consent/retention unresolved | Start in week 1; treat deployment as blocked, development as unblocked | Product/Legal |
| Spec contradiction: "no bare ALTER TABLE" vs ALTER statements | Migration review ambiguity | Interpret as "ALTER only inside versioned migrations"; record in REUSE_MAP deviations | CORE |
| Determinism claim unrealistic at temperature 0 | Re-run differs slightly | Define tolerance, log seeds/params, or relax gate with evidence | ANALYSIS |

---

## 11. Open decisions requiring human input

1. **Backend:** keep the Python FastAPI service or bypass it after RECON.
2. **Diarization:** which ONNX speaker-embedding model, and what licence.
3. **Model manifest signing:** key custody and rotation.
4. **Rubric criteria:** how sample answers are decomposed into checkable criteria.
5. **Reference hardware:** exact 8 GB macOS and Windows models for all measurements.
6. **Consent language and retention:** Legal-owned; blocks deployment.
7. **Corpus sourcing:** consented real calls vs synthetic; privacy review.
8. **Rating behaviour if the model underperforms:** scored vs advisory.
9. **Windows loopback fallback UX:** how loud the warning must be.
10. **Export format details:** exact Markdown structure and whether `.txt` mirrors it.

---

## 12. First ten actions

1. Import upstream meetily at a pinned commit; record the hash in `docs/REUSE_MAP.md`.
2. Create `AGENTS.md` with invariants, commands, and the chunk report template.
3. Add this plan and the source spec to `docs/`.
4. Launch RECON; produce `docs/REUSE_MAP.md`; hold Wave 1 until approved.
5. Open the Legal/consent review in parallel.
6. Start synthetic fixture design for the evaluation corpus.
7. Provision the two 8 GB reference machines.
8. Stand up the CI skeleton (fmt, clippy, lint, typecheck, audits).
9. Decide the diarization model and add it to the manifest design.
10. Freeze the initial model set and verify licences: `small.en`, `base.en`, embedding ONNX, Qwen2.5-3B.

---

## 13. Immediate acceptance criteria for this plan

- The team agrees that Wave 0 is blocking and that the Python backend decision is a hard prerequisite.
- The team accepts that gate evidence, not code volume, defines completion.
- The team agrees that the echo test, boundary diff, injection test, evidence citation, and full-disk search are non-negotiable.
- Legal and corpus work start in week 1, not at release.
