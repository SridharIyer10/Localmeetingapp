# CLAUDE.md

Read `AGENTS.md` first: commands, invariants, conventions. `README.md` describes the product;
`BUILDING.md` the build; `docs/V1_PLAN.md` the v1.0 architecture and cleanup.

- Stack: Tauri 2 (Rust) + Next.js 14 static export, macOS and Windows only.
- One recording/transcription engine (`src-tauri/src/audio`), one meeting screen
  (`frontend/src/components/meeting`), one session layer (`frontend/src/lib/meeting`).
  A meeting type is data on the session, never a separate code path.
- Do not edit or squash existing SQL migrations (installed databases checksum them); add new ones.
- Verify with the commands in `AGENTS.md` before calling anything done.
