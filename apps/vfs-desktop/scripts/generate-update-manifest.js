#!/usr/bin/env node
/** Tauri 2 release adapter: collect signed bundles and generate the static update feed. */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const PAYLOADS = Object.freeze({
  macos: { name: 'diaspor-vfs.app.tar.gz', suffix: '.app.tar.gz', targets: ['darwin-aarch64', 'darwin-x86_64'] },
  windows: { name: 'diaspor-vfs-updater.msi', suffix: '.msi', targets: ['windows-x86_64'] },
  linux: { name: 'diaspor-vfs-updater.AppImage', suffix: '.AppImage', targets: ['linux-x86_64'] },
});
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;
function signatureFor(file) {
  const signature = fs.readFileSync(file, 'utf8').trim();
  if (!signature) throw new Error('Updater signature is empty: ' + file);
  return signature;
}

/** Copy a single Tauri-generated payload/signature pair without changing its bytes. */
export function collectUpdaterArtifacts({ platform, bundleDir, outputDir }) {
  const spec = PAYLOADS[platform];
  if (!spec) throw new Error('Unsupported updater platform: ' + platform);
  const candidates = fs.existsSync(bundleDir) ? fs.readdirSync(bundleDir).filter(name =>
    name.endsWith(spec.suffix) && fs.statSync(path.join(bundleDir, name)).isFile() &&
    fs.existsSync(path.join(bundleDir, name + '.sig'))) : [];
  if (candidates.length !== 1) throw new Error('Expected exactly one signed ' + platform + ' updater payload');
  const input = path.join(bundleDir, candidates[0]);
  signatureFor(input + '.sig');
  fs.mkdirSync(outputDir, { recursive: true });
  for (const suffix of ['', '.sig']) {
    const destination = path.join(outputDir, spec.name + suffix);
    if (path.resolve(input + suffix) !== path.resolve(destination)) fs.copyFileSync(input + suffix, destination);
  }
}

/** Build a feed from signed payloads only. App version and release tag are independent. */
export function createManifest({ artifactsDir, version, tag, repository, date = new Date().toISOString() }) {
  if (typeof version !== 'string' || !SEMVER.test(version.replace(/^v/, ''))) throw new Error('A valid app version is required');
  if (typeof tag !== 'string' || !/^v?[0-9A-Za-z][0-9A-Za-z.+_-]*$/.test(tag)) throw new Error('A safe release tag is required');
  if (typeof repository !== 'string' || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) throw new Error('A valid repository owner/name is required');
  const manifest = { version: version.replace(/^v/, ''), notes: 'Release ' + tag, pub_date: date, platforms: {} };
  for (const [platform, spec] of Object.entries(PAYLOADS)) {
    const file = path.join(artifactsDir, platform, spec.name);
    if (!fs.existsSync(file) || !fs.existsSync(file + '.sig')) continue;
    const signature = signatureFor(file + '.sig');
    if (!fs.statSync(file).isFile() || fs.statSync(file).size === 0) throw new Error('Updater payload is empty: ' + file);
    const entry = { signature, url: 'https://github.com/' + repository + '/releases/download/' + encodeURIComponent(tag) + '/' + spec.name };
    for (const target of spec.targets) manifest.platforms[target] = { ...entry };
  }
  return Object.keys(manifest.platforms).length ? manifest : null;
}

/** No signed build means no feed, including removal of a stale generated output. */
export function writeManifest({ output, ...options }) {
  const manifest = createManifest(options);
  if (!manifest) {
    fs.rmSync(output, { force: true });
    console.warn('::warning title=Updater manifest skipped::No signed updater payloads were produced; normal installers remain available.');
    return false;
  }
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, JSON.stringify(manifest, null, 2) + '\n');
  return true;
}

function main(argv) {
  const command = argv.shift(), options = {};
  while (argv.length) {
    const key = argv.shift(), value = argv.shift();
    if (!key?.startsWith('--') || !value || value.startsWith('--') || Object.hasOwn(options, key)) throw new Error('Expected unique --option value arguments');
    options[key] = value;
  }
  if (command === 'collect') {
    for (const key of ['--platform', '--bundle-dir', '--output-dir']) if (!options[key]) throw new Error('Missing ' + key);
    collectUpdaterArtifacts({ platform: options['--platform'], bundleDir: options['--bundle-dir'], outputDir: options['--output-dir'] });
  } else if (command === 'manifest') {
    for (const key of ['--artifacts', '--output']) if (!options[key]) throw new Error('Missing ' + key);
    writeManifest({ artifactsDir: options['--artifacts'], output: options['--output'], version: options['--version'], tag: options['--tag'], repository: options['--repository'] });
  } else throw new Error('Usage: generate-update-manifest.js collect|manifest --option value');
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
