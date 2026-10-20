# Intake Pro v1.0 — unification + cleanup plan

Snapshot of the pre-change tree: `~/Documents/Intakepro-snapshots/pre-v1.0-*.tgz`.

## 0. Findings that shape the plan

| Finding | Consequence |
|---|---|
| Backend recording is already ONE path (`start_recording_with_devices_and_meeting` → `RecordingManager` → transcription worker). | The audio/STT engine is **not** touched (constraint: no recording regression). "MeetingSession" is the frontend/session layer over it. |
| Duplication is in the frontend: browser `/copilot` flow, `Home` branching on `newMeetingMode`, `sessionStorage` flags (`ACTIVE_COPILOT_MEETING_KEY`, `autoStartRecording`). | One `MeetingSessionProvider` + one `MeetingShell`; kind (`standard`/`copilot`) is data, not a code path. |
| `RecordingStateContext` already mirrors backend state (single source of truth for recording). | Session **phase** (idle/recording/paused/processing/done) is *derived* from it, never stored twice. |
| `AGENTS.md` invariants 1 and 6 / non-goal "no live question detection". | **Spec change requested by owner.** Live tagging is implemented with the rule-based analyser only (no LLM, no `llama-server` while capturing → invariant 1 holds). Invariant 6 is amended in `AGENTS.md`. Live tags are *provisional*; the final tags come from the post-meeting analysis. |
| Upstream leftovers: dead modules (`lib_old_complex.rs` 2437 lines, `audio_v2/`, `core-old.rs`, `recording_saver_old.rs`), updater pointing at the upstream fork's GitHub releases with upstream's key, Python `backend/` + Docker, Linux CI/cfgs, `vs_buildtools.exe`. | Deleted after usage search (§3). |
| SQL migrations are checksummed in users' DBs. | Migrations are **not** squashed/edited (would break existing installs). New migrations are additive only. |

## 1. Shared core

New (frontend):
- `src/lib/meeting/session.tsx` — `MeetingSessionProvider`, `useMeetingSession()`: `kind`, `questionSetId`, `questions`, live statuses, `phase` (derived), `begin()` (co-pilot: creates the draft meeting + copies the set's questions), `reset()`. Both kinds go through the same `begin → start → stop → finalize` sequence.
- `src/lib/meeting/phase.ts` — pure `derivePhase(status, isRecording, isPaused)` (unit-tested).
- `src/components/meeting/MeetingShell.tsx` — the one shell: header, optional left `QuestionPanel`, `TranscriptPanel`, `RecordingControls`, status overlays.
- `src/components/meeting/QuestionPanel.tsx` — scrollable, all questions, live status badge each, manual tick + notes.
- `src/components/meeting/NewMeeting.tsx` — kind chooser; co-pilot shows the question-set dropdown.
- `src/hooks/useLiveQuestionStatus.ts` — debounced, async, non-blocking.

Changed: `app/page.tsx` (chooser + shell only), `hooks/useRecordingStop.ts` (reads session, no `sessionStorage`), `layout.tsx` (remove web-demo branch).

## 2. Features

| # | Change | Files |
|---|---|---|
| 3 | Live status | Rust `copilot_live_status` (rule-based `copilot-core`, `spawn_blocking`), TS hook above |
| 4 | Post-meeting | Rust `copilot_run_analysis` uses the configured summary model via `summary::llm_client::generate_summary` through `copilot/app_llm.rs`, falls back to rules; stage polling `copilot_analysis_status`; TS `QuestionSummary` rendered above the narrative `SummaryPanel`; narrative stays a separate record (appended, never replacing); auto-narrative suppressed for co-pilot |
| 5 | CSV | `lib/copilot/csv.ts` (one schema `question_set,question,sample_questions,sample_answers`, RFC-4180, template, export, row-level validation), Rust `question_set_import` (create or replace, transactional) + `save_text_file`, UI in `QuestionSetManager` (Settings) and the question-set dropdown screen |
| 6 | Summary types | Custom types stored as templates in the existing custom-templates dir (`api_save_custom_template`, `api_delete_custom_template`, `is_custom` + `prompt` on list); all dropdowns already read `api_list_templates` so they get them for free |

## 3. Deletions (each verified by usage search first)

Rust: `lib_old_complex.rs`, `audio_v2/`, `audio/core-old.rs`, `audio/recording_saver_old.rs`, `whisper_engine/_stderr_suppressor.rs`, `copilot/summary_template.rs`, Linux `cfg` branches + `devices/platform/linux.rs`, updater plugin, unused Tauri commands.
Frontend: `app/copilot`, `app/demo`, `app/models`, `lib/copilot/{models,demo,export}.ts`, `ConsentGate`, `Teleprompter`, `QuestionIngest`, `QuestionLibrary`, `QuestionReport`, updater UI/services/hooks, web-only scripts.
Repo: `dist/IntakePro-Demo*`, `backend/` (Python server + Docker; the app is fully native), Linux/Docker/devtest workflows, `docs/building_in_linux.md`, `vs_buildtools.exe`, stray `logs/`, `*.backup`, GPU helper scripts for Linux, upstream docs.
Versions: single `1.0.0` in `[workspace.package]`; crates inherit; `tauri.conf.json` has no version (Tauri reads Cargo); `package.json` is checked against Cargo by `scripts/intake-audit.mjs`.

Deferred (explicitly): the analytics layer (118 call sites in 26 files, sends nothing because its key is empty) — see final report.

## 4. Verification
`cargo test -p copilot-core`, `cargo test -p intake-pro --lib`, `pnpm typecheck/lint/test`, `node scripts/intake-audit.mjs`, mocked-Tauri UI run, macOS `tauri build`, launch check. Windows: CI matrix added; cannot be executed on this Mac.
