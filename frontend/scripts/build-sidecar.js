#!/usr/bin/env node
/**
 * Builds the llama-helper sidecar and places it where Tauri's `externalBin`
 * expects it: frontend/src-tauri/binaries/llama-helper-<target-triple>[.exe].
 * Called by tauri-auto.js before `tauri dev|build`, so a fresh checkout builds
 * on macOS and Windows without a manual step.
 */
const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..', '..');
const triple = /^host: (.+)$/m.exec(execSync('rustc -vV', { encoding: 'utf8' }))[1].trim();
const exe = triple.includes('windows') ? '.exe' : '';

console.log(`Building llama-helper sidecar for ${triple}...`);
execSync('cargo build --release -p llama-helper', { cwd: root, stdio: 'inherit' });

const outDir = path.join(root, 'frontend', 'src-tauri', 'binaries');
fs.mkdirSync(outDir, { recursive: true });
fs.copyFileSync(
  path.join(root, 'target', 'release', `llama-helper${exe}`),
  path.join(outDir, `llama-helper-${triple}${exe}`),
);
console.log(`Sidecar ready: binaries/llama-helper-${triple}${exe}`);
