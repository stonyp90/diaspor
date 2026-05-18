#!/usr/bin/env node
/**
 * Generate update manifest for Tauri updater
 * 
 * This script creates a latest.json file that Tauri uses to check for updates.
 * It scans the artifacts directory and generates platform-specific download URLs.
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const VERSION = process.env.VERSION || process.argv[2] || '1.1.33';
const REPOSITORY = process.env.GITHUB_REPOSITORY || 'stonyp90/Ursly';
const BASE_URL = `https://github.com/${REPOSITORY}/releases/download/${VERSION}`;

// Clean version (remove 'v' prefix if present)
const cleanVersion = VERSION.startsWith('v') ? VERSION.slice(1) : VERSION;

// Check for artifacts
const artifactsDir = path.join(__dirname, '..', '..', '..', 'artifacts');
const manifest = {
  version: cleanVersion,
  notes: `Release ${cleanVersion}`,
  pub_date: new Date().toISOString(),
  platforms: {},
};

// Check for macOS artifact
const macosDmg = path.join(artifactsDir, 'macos', 'ursly-vfs.dmg');
if (fs.existsSync(macosDmg)) {
  const stats = fs.statSync(macosDmg);
  manifest.platforms['darwin-x86_64'] = {
    signature: '', // Will be filled by Tauri build process if signing is enabled
    url: `${BASE_URL}/ursly-vfs.dmg`,
    size: stats.size,
  };
  manifest.platforms['darwin-aarch64'] = {
    signature: '',
    url: `${BASE_URL}/ursly-vfs.dmg`,
    size: stats.size,
  };
  console.log(`✅ Found macOS artifact: ${stats.size} bytes`);
} else {
  console.log('⚠️  macOS artifact not found');
}

// Check for Windows artifact
const windowsMsi = path.join(artifactsDir, 'windows', 'ursly-vfs.msi');
if (fs.existsSync(windowsMsi)) {
  const stats = fs.statSync(windowsMsi);
  manifest.platforms['windows-x86_64'] = {
    signature: '', // Will be filled by Tauri build process if signing is enabled
    url: `${BASE_URL}/ursly-vfs.msi`,
    size: stats.size,
  };
  console.log(`✅ Found Windows artifact: ${stats.size} bytes`);
} else {
  console.log('⚠️  Windows artifact not found');
}

// Check for Linux artifact
const linuxAppImage = path.join(artifactsDir, 'linux', 'ursly-vfs.AppImage');
if (fs.existsSync(linuxAppImage)) {
  const stats = fs.statSync(linuxAppImage);
  manifest.platforms['linux-x86_64'] = {
    signature: '', // Will be filled by Tauri build process if signing is enabled
    url: `${BASE_URL}/ursly-vfs.AppImage`,
    size: stats.size,
  };
  console.log(`✅ Found Linux artifact: ${stats.size} bytes`);
} else {
  console.log('⚠️  Linux artifact not found');
}

// Write manifest
const outputPath = path.join(__dirname, '..', 'latest.json');
fs.writeFileSync(outputPath, JSON.stringify(manifest, null, 2));

console.log(`\n✅ Update manifest generated: ${outputPath}`);
console.log(`📦 Version: ${cleanVersion}`);
console.log(`🌐 Platforms: ${Object.keys(manifest.platforms).join(', ') || 'none'}`);
console.log(`\n${JSON.stringify(manifest, null, 2)}`);

// Exit with error if no platforms found
if (Object.keys(manifest.platforms).length === 0) {
  console.error('\n❌ Warning: No platform artifacts found. Manifest generated but may not be useful.');
  process.exit(0); // Don't fail the build, just warn
}
