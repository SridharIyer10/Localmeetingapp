# REUSE_MAP — Intake Co-Pilot

**Chunk 0 deliverable.** Read-only reconnaissance of the forked meetily baseline.
**Pinned upstream commit:** `a2cb62e827da7ef59f65064c97233efb2313878e` (`Release v0.4.1`)
**Licence:** MIT (`LICENSE.md`)
**Author:** RECON
**Date:** 2026-10-19

> Every claim below cites a repository path and either a line range or a named
> function. Line numbers refer to the pinned commit.

---

## 1. Audio capture

| Concern | Where | Notes |
|---|---|---|
| Device discovery | `frontend/src-tauri/src/audio/devices/discovery.rs`, `devices/microphone.rs`, `devices/speakers.rs` | `list_audio_devices`, default device helpers |
| Microphone capture | `frontend/src-tauri/src/audio/capture/microphone.rs` | cpal input stream |
| System capture (macOS) | `frontend/src-tauri/src/audio/capture/core_audio.rs`, `capture/system.rs` | ScreenCaptureKit path |
| System capture (Windows) | `frontend/src-tauri/src/audio/capture/system.rs`, `audio/system_audio_stream.rs` | WASAPI path |
| Mixing / ring buffer | `frontend/src-tauri/src/audio/pipeline.rs` (`AudioMixerRingBuffer`, `ProfessionalAudioMixer`) | 600 ms mixing window, zero-pad, ducking |
| VAD | `frontend/src-tauri/src/audio/vad.rs` (`ContinuousVadProcessor`) | Silero via `silero_rs`, 16 kHz, 30 ms frames |
| Recording orchestration | `frontend/src-tauri/src/audio/recording_manager.rs`, `recording_commands.rs`, `recording_state.rs` | Tauri commands and state |
| Incremental save/recovery | `frontend/src-tauri/src/audio/incremental_saver.rs` | Checkpointed audio recovery |
| Permissions | `frontend/src-tauri/src/audio/permissions.rs`, `check_screen_permission.swift` | macOS permission handling |

**Current chunk model:** meetily captures and mixes continuously; VAD emits *speech
segments* (`vad.rs::SpeechSegment`) of variable length, not fixed ~30 s chunks.
Speech segments are streamed to the transcription engine. There is no disk chunk
layer and no encryption.

**Gap for Chunk 2:** fixed-duration VAD-aligned chunking (~30 s, 2 s overlap),
XChaCha20 encryption, session-key lifecycle, cache-directory enforcement,
backup/sync exclusion, echo suppression, discard-last-30s, Bluetooth HFP warning.

**Action:** extend `pipeline.rs` / `vad.rs` and add a chunk sink. Do not rewrite capture.

---

## 2. Transcription

| Concern | Where | Notes |
|---|---|---|
| Engine abstraction | `frontend/src-tauri/src/audio/transcription/engine.rs`, `provider.rs` | Whisper and Parakeet providers |
| Whisper provider | `frontend/src-tauri/src/audio/transcription/whisper_provider.rs` | wraps `whisper_engine` |
| Worker pool | `frontend/src-tauri/src/audio/transcription/worker.rs` (`start_transcription_task`) | single serial worker, emits `transcript-update` |
| Whisper engine | `frontend/src-tauri/src/whisper_engine/whisper_engine.rs`, `commands.rs`, `parallel_processor.rs` | model load, transcribe, GPU accel |
| VAD gating | `audio/vad.rs`, used from `audio/pipeline.rs` | already mandatory on the live path |
| Persistence of segments | `frontend/src-tauri/src/database/repositories/transcript.rs` (`save_transcript`) | rows into `transcripts` table |

**Streaming vs batch:** live transcription is streaming-ish: VAD speech segments
are transcribed as they close during recording and emitted as
`transcript-update` events. This already matches the spec's "transcribe during the
call" requirement.

**Where output lands:** in-memory events plus the `transcripts` table
(`transcripts.transcript`, `audio_start_time`, `audio_end_time`, `duration`).

**Gap for Chunk 4:** `transcript_segments` table with `seq`, `channel`, and
`speaker_label`; overlap reconciliation; deterministic ordering; offline speaker
clustering; startup recovery for untranscribed chunks; `transcription_completed_at`.

**Action:** extend the worker to also write `transcript_segments`; keep the existing
engine bindings.

---

## 3. Persistence

| Concern | Where | Notes |
|---|---|---|
| Pool + migration | `frontend/src-tauri/src/database/manager.rs` (`DatabaseManager::new`) | `SqlitePool::connect` then `sqlx::migrate!("./migrations")` |
| Migrations | `frontend/src-tauri/migrations/*.sql` | sqlx embedded, 12 migrations |
| Schema (initial) | `migrations/20250916100000_initial_schema.sql` | `meetings`, `transcripts`, `summary_processes`, `transcript_chunks`, `settings`, `transcript_settings` |
| Models | `frontend/src-tauri/src/database/models.rs` | `MeetingModel`, `Transcript`, `Setting`, ... |
| Repositories | `frontend/src-tauri/src/database/repositories/{meeting,transcript,setting,summary,transcript_chunk}.rs` | meeting/transcript CRUD |
| Setup | `frontend/src-tauri/src/database/setup.rs` (`initialize_database_on_startup`) | first-launch detection |

**Migration mechanism:** sqlx migrations exist and are adequate. Add a versioned
migration; do not build a second runner. The `schema_version` table named in the
spec is not required given sqlx already tracks `_sqlx_migrations`; record this as a
deviation.

**Pragmas in force:** `manager.rs` opens with `SqlitePool::connect` and does not set
`foreign_keys` or `journal_mode`. FKs are declared in migrations but SQLite does not
enforce them by default, so `ON DELETE CASCADE` is currently inert.

**Gap for Chunk 1:** enable `foreign_keys=ON` and `journal_mode=WAL` on every
connection; add copilot columns and tables; enable SQLCipher.

**Action:** extend `manager.rs` with `SqliteConnectOptions`; add one migration.

---

## 4. Backend — is the Python FastAPI service load-bearing?

**No. It can be bypassed.** This is the decisive answer.

Evidence:
- `frontend/src-tauri/src/api/api.rs:20` defines `APP_SERVER_URL = "http://localhost:5167"`,
  but meeting, transcript, settings and config commands use the Rust SQLite
  repositories directly (`api.rs:331-1015`, e.g. `MeetingsRepository::get_meetings`,
  `TranscriptsRepository::search_transcripts`, `SettingsRepository::*`).
- The only HTTP uses of `APP_SERVER_URL` are legacy profile/licence endpoints
  (`api_get_profile`, `api_save_profile`, `api_update_profile`) and a
  `test_backend_connection` / `debug_backend_connection` diagnostic.
- `backend/app/main.py` implements meeting/summary endpoints and an
  `allow_origins=["*"]` CORS policy (`backend/app/main.py:50-56`), but the Rust app
  does not depend on those for core behaviour.
- The sidebar calls Tauri commands (`frontend/src/components/Sidebar/SidebarProvider.tsx:100,188`),
  not FastAPI.

**Consequence for §4.3:** embeddings run in Rust with `ort` + `tokenizers`. The
Python backend, `pandas`, and `python-multipart` are not required. The Intake
Co-Pilot should not start or ship FastAPI.

**Action:** bypass. Do not add endpoints to `backend/`. Treat licensing/profile as
out of scope for v1.

---

## 5. Frontend

| Concern | Where | Notes |
|---|---|---|
| App shell / routes | `frontend/src/app/` (`page.tsx`, `layout.tsx`, `settings/page.tsx`, `meeting-details/`, `notes/[id]/`) | Next.js App Router |
| Global state | `frontend/src/components/Sidebar/SidebarProvider.tsx` | meetings, current meeting, recording status |
| IPC convention | `@tauri-apps/api` `invoke` + `listen` | commands registered in `lib.rs:612+` |
| Recording UI | `frontend/src/components/RecordingControls.tsx`, `RecordingStatusBar.tsx`, `DeviceSelection.tsx` | |
| Transcript UI | `frontend/src/components/VirtualizedTranscriptView.tsx`, `MeetingDetails/TranscriptPanel.tsx` | virtualized |
| Settings UI | `frontend/src/components/SettingTabs.tsx`, `PreferenceSettings.tsx`, `WhisperModelManager.tsx`, `BuiltInModelManager.tsx` | |
| Dialogs | `frontend/src/components/ConfirmationModel/confirmation-modal.tsx`, `CustomDialog.tsx` | reusable |
| Styles | Tailwind + shadcn-style primitives | |

**Gap:** no question ingest, no teleprompter, no consent gate, no report view, no
per-question tick-off.

**Action:** add a `/copilot` route and `components/copilot/*`; reuse dialogs and
Tailwind primitives.

**Pre-existing unsafe HTML:** `frontend/src/app/notes/[id]/page.tsx:174` uses
`dangerouslySetInnerHTML`. Not on the copilot path, but Chunk 10's grep will flag it.
Recorded as a deviation to triage, not silently ignored.

---

## 6. Model handling

| Concern | Where | Notes |
|---|---|---|
| Whisper model mgmt | `frontend/src-tauri/src/whisper_engine/commands.rs`, `parallel_commands.rs`; UI `WhisperModelManager.tsx`, `ModelDownloadProgress.tsx` | download/validate/delete |
| Parakeet model mgmt | `frontend/src-tauri/src/parakeet_engine/model.rs`, `commands.rs`; UI `ParakeetModelManager.tsx` | ONNX-based |
| Built-in LLM (summary) | `frontend/src-tauri/src/summary/summary_engine/model_manager.rs`, `models.rs`, `commands.rs`; UI `BuiltInModelManager.tsx` | downloads GGUF, uploads |
| Sidecar | `frontend/src-tauri/src/summary/summary_engine/sidecar.rs` | manages `llama-helper` child process (stdin/stdout protocol) |
| LLM clients | `summary/llm_client.rs`, `ollama/`, `openai/`, `anthropic/`, `groq/`, `openrouter/` | multiple providers |

**Important:** the existing sidecar is `llama-helper` (a bundled Rust child using
stdout/stdin), **not** `llama-server`. The spec assumes `llama-server`. Also, there
is no signed manifest, no SHA-256 verification surfaced here, and no lifecycle rule
preventing the helper from running during capture.

**Gap for Chunk 3:** signed manifest with digests, verification before every load,
`llama-server` (or the helper) lifecycle gated to post-processing only, hardware
probe, licence disclosure.

**Action:** add a manifest + verifier; extend the supervisor with a hard
"never while capturing" gate.

---

## 7. Security posture as found

| Item | Finding |
|---|---|
| Local service | Rust app has no local HTTP listener; Python FastAPI binds `localhost:5167` with `allow_origins=["*"]` (`backend/app/main.py:50-56`). |
| Sidecar communication | `llama-helper` uses stdin/stdout (`summary_engine/sidecar.rs`), not a TCP port. No API key needed for the helper protocol. |
| Secrets | API keys are stored in the SQLite `settings` / `transcript_settings` columns (`migrations/20250916100000_initial_schema.sql`) and read via `SettingsRepository` (`api.rs:584,697`). Keys are also fetched into process memory. No OS keychain. |
| Logging | `lib.rs` defines zero-cost `perf_debug!`/`perf_trace!`. Transcript text is emitted in `transcript-update` events; check log level before shipping. |
| CSP | `frontend/src-tauri/tauri.conf.json` `connect-src` allowlists `localhost:11434`, `localhost:5167`, `localhost:8178`, `api.ollama.ai`; `fs:read-all`/`fs:write-all` are granted to the main window. |
| Analytics | `posthog-rs` dependency plus `analytics/` module — must be confirmed to not transmit transcript content. |

**Gap for Chunk 1/10:** loopback-only token-gated service (only if we keep any local
HTTP), keychain-backed secret store, no API key in DB columns, tightened CSP and FS
scopes, transcript-safe logging.

---

## 8. Reuse table (chunks 1–10)

| Chunk | Extend (existing) | Must be new | Reason |
|---|---|---|---|
| 1 Foundation | `database/manager.rs`, `database/models.rs`, `database/repositories/*`, `migrations/` | copilot migration; secret-store module; local-auth module | sqlx migration runner already exists; only the schema/crypto/auth are missing |
| 2 Audio | `audio/pipeline.rs`, `audio/vad.rs`, `audio/capture/*`, `audio/permissions.rs` | chunk sink, crypto module, echo suppressor, lifecycle controls | capture already works; chunking/encryption/privacy do not exist |
| 3 Runtime | `summary/summary_engine/model_manager.rs`, `sidecar.rs`, `whisper_engine/*` | manifest + verifier, lifecycle gate, hardware probe | download code exists but unsigned and ungated |
| 4 STT | `audio/transcription/worker.rs`, `whisper_engine/*`, `database/repositories/transcript.rs` | segment reconciler, diarizer, recovery sweep | engine bindings exist; segment-level reconcile/diarization do not |
| 5 UI ingest | `frontend/src/app/*`, `frontend/src/components/*`, `ConfirmationModel/*` | `/copilot` route, teleprompter, ingest parser, consent gate | no question features exist |
| 6 Analysis | `database/repositories/*`; embed via `ort` (already a dependency) | windowing, BM25+cosine retrieval, evaluator, notes extractor, checkpoint | nothing similar exists |
| 7 Report | `frontend/src/components/*`, `VirtualizedTranscriptView.tsx` | report view, correction UI, MD/text renderers | no report exists |
| 8 Lifecycle | `audio/recording_manager.rs`, `database/repositories/meeting.rs` | key zeroize, chunk unlink, startup sweep, audit log | no crypto/shred concept exists |
| 9 Eval | none | fixture corpus, runner, metrics, gate script | nothing exists |
| 10 Security | `.github/workflows/*`, `entitlements.plist`, `scripts/sign-windows.ps1`, `tauri.conf.json` | audit evidence, updater pinning, licence file | pipelines exist; copilot controls must be verified |

---

## 9. Deviations from the spec

1. **`schema_version` table:** the spec mandates it, but sqlx already tracks versions
   in `_sqlx_migrations`. Adding a second version table duplicates state. Proposal:
   rely on sqlx and document it. Needs human sign-off.
2. **Migration contradiction:** §3.4 says "No bare ALTER TABLE" yet the schema uses
   `ALTER TABLE meetings ADD COLUMN`. Interpretation: ALTER is allowed only inside a
   versioned migration. Recorded for the reviewer.
3. **Sidecar is `llama-helper`, not `llama-server`:** the spec's `llama-server`
   flags (`-c 4096`, `--api-key`, JSON-schema/GBNF) do not map one-to-one. Either
   add `llama-server` as a new sidecar or extend `llama-helper`. Needs a decision.
4. **Existing API-key DB columns:** `settings`/`transcript_settings` hold keys. The
   spec forbids keys in DB columns. Migration must move keys to the keychain and
   null the columns.
5. **Existing analytics (`posthog`):** must be audited so no transcript content is
   transmitted; the spec implies a privacy-first posture.
6. **`dangerouslySetInnerHTML` exists** at `frontend/src/app/notes/[id]/page.tsx:174`
   (not on the copilot path).
7. **CSP / FS scope are broad** (`fs:read-all`, `fs:write-all`); copilot additions
   should not widen them further.
8. **No fixed audio chunks on disk** today; the spec's ~30 s chunk model is new work,
   not an extension of an existing chunk file format.
9. **Diarization model is unspecified** in the spec; meetily has no diarization. An
   ONNX speaker-embedding model must be chosen (no torch allowed).
10. **`audio/vad.rs` uses `silero_rs` from git**, while the spec says "Silero VAD
    (ONNX, ~1 MB)". This is already ONNX under the hood, but the dependency is a
    git pin rather than the manifest; note for supply-chain review.

---

## 10. Blocking decision required from a human

- Approve deviation 1 (sqlx version tracking instead of a second `schema_version`).
- Choose sidecar strategy (deviation 3).
- Confirm the diarization model and its licence (deviation 9).
