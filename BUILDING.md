# Building Intake Pro (macOS and Windows)

Only macOS (Apple Silicon) and Windows (x64) are supported.

## Prerequisites

| | macOS | Windows |
|---|---|---|
| Toolchain | Full **Xcode** (the capture dependency builds an Xcode project), `xcode-select -s /Applications/Xcode.app/Contents/Developer` | **Visual Studio Build Tools** (C++ workload) and **CMake** |
| Common | Rust (stable), Node 20+, pnpm | Rust (stable), Node 20+, pnpm |

## Build

```bash
cd frontend
pnpm install
pnpm tauri:build        # builds the llama-helper sidecar, the web UI and the app
```

Outputs are in `target/release/bundle/`: `.app` and `.dmg` on macOS, `.exe` (NSIS) and
`.msi` on Windows. FFmpeg and (on Windows) ONNX Runtime are downloaded at build time.

Develop with `pnpm tauri:dev`.

Notes:
- macOS builds are ad-hoc signed; notarisation needs an Apple Developer ID.
- Windows signing is skipped unless `DIGICERT_KEYPAIR_ALIAS` is set (`src-tauri/scripts/sign-windows.ps1`).
- On Windows, set `CMAKE_PROJECT_INCLUDE` to `.github/force-portable-ggml.cmake` before building
  so whisper.cpp targets the x64 baseline, and run `node .github/verify-portable-ggml.cjs target/release`
  before shipping. CI does both.
- Optional GPU builds: `--features cuda` or `--features vulkan` (Windows), `--features coreml` (macOS).
- Never set `DOCS_RS=1` for a real build (it skips the macOS Xcode step and fails at link time).

## Versioning

The version is defined once, in the workspace `Cargo.toml` (`[workspace.package] version`).
The crates inherit it, `tauri.conf.json` has no version (Tauri reads Cargo), and
`frontend/package.json` must match it; `scripts/intake-audit.mjs` enforces that.
