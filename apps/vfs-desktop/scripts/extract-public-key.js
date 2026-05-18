#!/usr/bin/env node
/**
 * Extract public key from Tauri signing private key
 * 
 * This script reads a Tauri signing private key and extracts the public key
 * for use in tauri.conf.json for auto-updates.
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { execSync } from 'child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const KEY_PATH = path.join(__dirname, '..', '.tauri-keys', 'key');

try {
  if (!fs.existsSync(KEY_PATH)) {
    console.error('❌ Error: Private key file not found at', KEY_PATH);
    process.exit(1);
  }

  const privateKey = fs.readFileSync(KEY_PATH, 'utf8').trim();
  
  if (!privateKey) {
    console.error('❌ Error: Private key file is empty');
    process.exit(1);
  }

  // Check if it's a valid Ed25519 private key format
  if (!privateKey.includes('BEGIN') || !privateKey.includes('PRIVATE')) {
    console.error('❌ Error: Invalid private key format');
    process.exit(1);
  }

  // Try to extract public key using Tauri CLI
  // The tauri signer command can extract the public key from a private key
  try {
    // Use npx to run tauri CLI (works in CI environments)
    const output = execSync(`npx @tauri-apps/cli signer generate -k "${KEY_PATH}"`, {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
      cwd: path.join(__dirname, '..'),
    });
    
    // Parse the output to extract the public key
    // The output format is typically:
    // Public Key: <key>
    const publicKeyMatch = output.match(/Public Key:\s*([^\s]+)/i);
    if (publicKeyMatch && publicKeyMatch[1]) {
      const publicKey = publicKeyMatch[1].trim();
      console.log('Public Key:');
      console.log(publicKey);
      process.exit(0);
    }
    
    // Alternative: try to find the key in the output
    const lines = output.split('\n');
    for (const line of lines) {
      const trimmed = line.trim();
      // Ed25519 public keys are typically base64 encoded and start with specific patterns
      if (trimmed.length > 40 && /^[A-Za-z0-9+/=]+$/.test(trimmed)) {
        console.log('Public Key:');
        console.log(trimmed);
        process.exit(0);
      }
    }
    
    console.error('❌ Error: Could not parse public key from Tauri CLI output');
    console.error('Output:', output);
    process.exit(1);
  } catch (cliError) {
    // If CLI extraction fails, we'll skip it and let the build continue
    // The build will work without the public key, but auto-updates won't be signed
    console.warn('⚠️  Warning: Could not extract public key using Tauri CLI');
    console.warn('⚠️  Error:', cliError.message);
    console.warn('⚠️  Build will continue, but auto-updates may not work');
    console.log('Public Key:');
    console.log(''); // Empty key - build will handle this gracefully
    process.exit(0); // Exit with success so build continues
  }
  
} catch (error) {
  console.error('❌ Error extracting public key:', error.message);
  // Don't fail the build if key extraction fails
  console.log('Public Key:');
  console.log(''); // Empty key
  process.exit(0);
}
