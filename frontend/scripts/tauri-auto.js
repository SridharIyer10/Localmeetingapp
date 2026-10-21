#!/usr/bin/env node
/**
 * Auto-detect GPU and run Tauri with appropriate features.
 * On macOS `build`, performs inside-out ad-hoc signing with entitlements,
 * strips extended attributes, and packages `dist/Intake Pro.app` + `dist/Intake Pro.dmg`
 * with a first-launch helper for local sharing over Email or AirDrop.
 */

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

// Get the command (dev or build)
const command = process.argv[2];
if (!command || !['dev', 'build'].includes(command)) {
  console.error('Usage: node tauri-auto.js [dev|build]');
  process.exit(1);
}

// Detect GPU feature
let feature = '';

if (process.env.TAURI_GPU_FEATURE) {
  feature = process.env.TAURI_GPU_FEATURE;
  console.log(`🔧 Using forced GPU feature from environment: ${feature}`);
} else {
  try {
    const result = execSync('node scripts/auto-detect-gpu.js', {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'inherit']
    });
    feature = result.trim();
  } catch (err) {
    // If detection fails, continue with no features
  }
}

console.log('');

const platform = os.platform();
const env = { ...process.env };

if (platform === 'linux' && feature === 'cuda') {
  console.log('🐧 Linux/CUDA detected: Setting CMAKE flags for NVIDIA GPU');
  env.CMAKE_CUDA_ARCHITECTURES = '75';
  env.CMAKE_CUDA_STANDARD = '17';
  env.CMAKE_POSITION_INDEPENDENT_CODE = 'ON';
}

// On macOS local builds, build the .app bundle via Tauri and package the DMG
// directly with hdiutil so Finder AppleScript in create-dmg never fails in headless/sandbox sessions.
let tauriCmd = `tauri ${command}`;
if (command === 'build' && platform === 'darwin') {
  tauriCmd += ' --bundles app';
}
if (feature && feature !== 'none') {
  tauriCmd += ` -- --features ${feature}`;
  console.log(`🚀 Running: ${tauriCmd}`);
} else {
  console.log(`🚀 Running: ${tauriCmd} (CPU-only mode)`);
}
console.log('');

function signAndPackageMacOS() {
  const repoRoot = path.resolve(__dirname, '..', '..');
  const appPath = path.join(repoRoot, 'target', 'release', 'bundle', 'macos', 'Intake Pro.app');
  const entitlementsPath = path.join(__dirname, '..', 'src-tauri', 'entitlements.plist');

  if (!fs.existsSync(appPath)) {
    console.warn(`⚠️ App bundle not found at ${appPath}`);
    return;
  }

  console.log('\n🔐 Signing Intake Pro.app for local macOS distribution (ad-hoc + entitlements)...');

  // 1. Strip any extended/provenance/quarantine attributes before signing
  try {
    execSync(`xattr -cr "${appPath}"`, { stdio: 'inherit' });
  } catch (_) {}

  // 2. Ensure +x on all binaries in Contents/MacOS and sign inside-out
  const macosBinDir = path.join(appPath, 'Contents', 'MacOS');
  for (const binName of ['ffmpeg', 'llama-helper', 'intake-pro']) {
    const binPath = path.join(macosBinDir, binName);
    if (fs.existsSync(binPath)) {
      fs.chmodSync(binPath, 0o755);
      execSync(
        `codesign --force --sign - --entitlements "${entitlementsPath}" --timestamp=none "${binPath}"`,
        { stdio: 'inherit' }
      );
    }
  }

  // 3. Sign top-level .app bundle and verify deep signature
  execSync(
    `codesign --force --sign - --entitlements "${entitlementsPath}" --timestamp=none "${appPath}"`,
    { stdio: 'inherit' }
  );
  execSync(`codesign --verify --deep --strict --verbose=2 "${appPath}"`, { stdio: 'inherit' });
  console.log('✅ Signature verified on Intake Pro.app');

  // 4. Copy to dist/ and create a local-sharing DMG with a first-launch helper
  const distDir = path.join(repoRoot, 'dist');
  const dmgBundleDir = path.join(repoRoot, 'target', 'release', 'bundle', 'dmg');
  const stagingDir = path.join(repoRoot, 'target', 'release', 'bundle', 'dmg-staging');

  fs.mkdirSync(distDir, { recursive: true });
  fs.mkdirSync(dmgBundleDir, { recursive: true });
  fs.rmSync(stagingDir, { recursive: true, force: true });
  fs.mkdirSync(stagingDir, { recursive: true });

  const distAppPath = path.join(distDir, 'Intake Pro.app');
  fs.rmSync(distAppPath, { recursive: true, force: true });
  execSync(`cp -R "${appPath}" "${distAppPath}"`, { stdio: 'inherit' });

  const stagingAppPath = path.join(stagingDir, 'Intake Pro.app');
  execSync(`cp -R "${appPath}" "${stagingAppPath}"`, { stdio: 'inherit' });

  try {
    fs.symlinkSync('/Applications', path.join(stagingDir, 'Applications'));
  } catch (_) {}

  const firstLaunchScript = [
    '#!/bin/bash',
    '# Installs Intake Pro to /Applications, clears macOS Email/AirDrop quarantine, and launches the app.',
    'set -e',
    'DIR="$(cd "$(dirname "$0")" && pwd)"',
    'SRC_APP="$DIR/Intake Pro.app"',
    'DEST_APP="/Applications/Intake Pro.app"',
    '',
    'echo "Installing Intake Pro..."',
    'if [ -d "$SRC_APP" ]; then',
    '  if [ -w "/Applications" ]; then',
    '    rm -rf "$DEST_APP"',
    '    cp -R "$SRC_APP" "$DEST_APP"',
    '    xattr -cr "$DEST_APP" 2>/dev/null || true',
    '    chmod -R +x "$DEST_APP/Contents/MacOS" 2>/dev/null || true',
    '    echo "✅ Installed to /Applications/Intake Pro.app"',
    '    open "$DEST_APP"',
    '    exit 0',
    '  else',
    '    xattr -cr "$SRC_APP" 2>/dev/null || true',
    '    chmod -R +x "$SRC_APP/Contents/MacOS" 2>/dev/null || true',
    '    open "$SRC_APP"',
    '    exit 0',
    '  fi',
    'elif [ -d "$DEST_APP" ]; then',
    '  xattr -cr "$DEST_APP" 2>/dev/null || true',
    '  chmod -R +x "$DEST_APP/Contents/MacOS" 2>/dev/null || true',
    '  open "$DEST_APP"',
    '  exit 0',
    'else',
    '  echo "Could not find Intake Pro.app in $DIR or /Applications."',
    '  exit 1',
    'fi',
    ''
  ].join('\n');

  const readmeText = [
    'Intake Pro — Running on Another Mac (Shared via Email or AirDrop)',
    '=================================================================',
    '',
    'Because Intake Pro is distributed directly (without Apple App Store notarization),',
    'macOS adds a quarantine flag when the app is sent over Email or AirDrop.',
    '',
    'Option 1 — Terminal One-Liner (Fastest):',
    '  1. Drag "Intake Pro.app" into your Applications folder.',
    '  2. Open Terminal and run:',
    '     xattr -cr "/Applications/Intake Pro.app" && open "/Applications/Intake Pro.app"',
    '',
    'Option 2 — Using the Included Installer Script:',
    '  1. Right-click (Control-click) "Open Intake Pro (First Time).command" and choose "Open".',
    '  2. Click "Open" in the dialog — it will copy Intake Pro to Applications, remove',
    '     the quarantine flag, and launch the app.',
    '',
    'Option 3 — macOS System Settings:',
    '  1. Drag "Intake Pro.app" to Applications and try to open it.',
    '  2. Open System Settings -> Privacy & Security, scroll down to Security,',
    '     and click "Open Anyway" next to Intake Pro.',
    ''
  ].join('\n');

  for (const targetFolder of [stagingDir, distDir]) {
    const cmdFile = path.join(targetFolder, 'Open Intake Pro (First Time).command');
    fs.writeFileSync(cmdFile, firstLaunchScript, { mode: 0o755 });
    fs.chmodSync(cmdFile, 0o755);
    fs.writeFileSync(path.join(targetFolder, 'READ ME FIRST (Mac Install).txt'), readmeText);
  }

  const dmgPath = path.join(distDir, 'Intake Pro.dmg');
  const versionedDmgPath = path.join(distDir, 'Intake Pro_1.0.0_aarch64.dmg');
  const bundleDmgPath = path.join(dmgBundleDir, 'Intake Pro_1.0.0_aarch64.dmg');

  try {
    fs.rmSync(dmgPath, { force: true });
    execSync(
      `hdiutil create -volname "Intake Pro" -srcfolder "${stagingDir}" -ov -format UDZO "${dmgPath}"`,
      { stdio: 'inherit' }
    );
    fs.copyFileSync(dmgPath, versionedDmgPath);
    fs.copyFileSync(dmgPath, bundleDmgPath);
    console.log(`✅ Created DMG at ${dmgPath}`);
  } catch (err) {
    console.warn(`⚠️ Could not create DMG automatically (${err.message}). Intake Pro.app is ready in dist/.`);
  } finally {
    fs.rmSync(stagingDir, { recursive: true, force: true });
  }
}

try {
  execSync('node scripts/build-sidecar.js', { stdio: 'inherit' });
  execSync(tauriCmd, { stdio: 'inherit', env });
  if (command === 'build' && platform === 'darwin') {
    signAndPackageMacOS();
  }
} catch (err) {
  process.exit(err.status || 1);
}
