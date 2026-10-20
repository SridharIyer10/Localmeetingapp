# Intake Pro v1.0 — feature status

Legend: **Done** = implemented and verified by tests/build · **Partial** =
implemented but not run end-to-end · **No** = not implemented.

## Chunk-by-chunk

| # | Area | Status | Notes |
|---|---|---|---|
| 0 | Recon / reuse map | Done | `docs/REUSE_MAP.md` |
| 1 | Schema, migrations, pragmas, secret store, local auth | Done* | `question_bank` + copilot tables; `foreign_keys=ON`/WAL; keychain; loopback token auth. Runtime DB gates pending GUI run |
| 2 | Audio capture, VAD chunking, encryption, echo | Partial | Pure modules tested (`audio_chunk`, `audio_crypto`, `echo`); not yet wired to the live capture path |
| 3 | Model manifest, downloader, sidecar lifecycle | Partial | `model_manifest` (SHA-256, fail-closed), `downloader` state machine, `lifecycle_gate` tested; app uses `llama-helper` |
| 4 | Transcription pipeline, reconcile, diarization | Partial | `reconcile` + `diarization` pure logic tested; live integration pending |
| 5 | Question sets + live questions | Done | Named sets, add/edit, CSV template/export/import with row-level validation, live per-question tags beside the transcript (mocked-Tauri UI run) |
| 6 | Analysis engine | Partial | Retrieval/rubric/prompt/schema/engine + heuristic fallback run and tested, including a DB-level bank → recording → finalize → analysis → verdict test; ONNX embedder + `llama-server` client code-complete, not run against real models |
| 7 | Post-meeting summary | Done | Automatic question summary (status + extracted answer + evidence, correctable) above the normal summary pane; narrative summary by chosen summary type is a separate explicit step |
| 8 | Audio lifecycle & disposal | Partial | `SessionKey` crypto-shred tested; disposal flow not wired to UI |
| 9 | Evaluation harness | Done* | `copilot-eval` + `--degrade`; metrics (P/R, accuracy, κ) — corpus is synthetic |
| 10 | Security audit & packaging | Partial | `scripts/intake-audit.mjs` 11 invariants; ad-hoc signed `.app`/`.dmg`; notarization not done |

\* Done at the source/unit level; GUI-dependent runtime gates still need a manual pass.

## Non-goals respected
- No LLM during recording. Live question tags are rule-based (owner-approved amendment of invariant 6).
- No meeting bot, cloud transcription or audio upload.
- English only; single user; no ATS/CRM write-back; no `torch`/`sentence-transformers`.

## Known gaps that block "complete"
1. Real model end-to-end: place the two models and run a recorded call through
   `analysis.rs` (ONNX embeddings + LLM).
2. Live capture integration for the tested audio/STT/lifecycle primitives.
3. Notarization (needs Apple Developer ID).
4. Honest evaluation baseline on real recordings.
