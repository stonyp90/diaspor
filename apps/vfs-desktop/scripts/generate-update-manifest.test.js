import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createManifest, collectUpdaterArtifacts, writeManifest } from './generate-update-manifest.js';

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'diaspor-updater-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}
function signed(root, platform, name, signature = 'signed-payload') {
  const dir = path.join(root, platform);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, name), 'binary bytes');
  fs.writeFileSync(path.join(dir, name + '.sig'), signature);
}
const options = root => ({ artifactsDir: root, version: '2.0.7', tag: 'v0.1.0-alpha.18', repository: 'stonyp90/diaspor', date: '2026-10-15T12:00:00.000Z' });

test('manifest uses app version, release tag, exact payloads and signature contents', t => {
  const root = fixture(t);
  signed(root, 'macos', 'diaspor-vfs.app.tar.gz', ' mac-signature\n');
  signed(root, 'windows', 'diaspor-vfs-updater.msi', 'win-signature');
  signed(root, 'linux', 'diaspor-vfs-updater.AppImage', 'linux-signature');
  const result = createManifest(options(root));
  assert.equal(result.version, '2.0.7');
  assert.equal(Object.keys(result.platforms).length, 4);
  assert.equal(result.platforms['darwin-aarch64'].signature, 'mac-signature');
  assert.deepEqual(result.platforms['darwin-aarch64'], result.platforms['darwin-x86_64']);
  assert.equal(result.platforms['darwin-aarch64'].url, 'https://github.com/stonyp90/diaspor/releases/download/v0.1.0-alpha.18/diaspor-vfs.app.tar.gz');
  assert.equal(result.platforms['windows-x86_64'].signature, 'win-signature');
});

test('unsigned installers never produce an updater manifest and stale output is removed', t => {
  const root = fixture(t), output = path.join(root, 'latest.json');
  fs.mkdirSync(path.join(root, 'macos'));
  fs.writeFileSync(path.join(root, 'macos', 'diaspor-vfs.dmg'), 'installer');
  fs.writeFileSync(output, 'stale');
  assert.equal(writeManifest({ ...options(root), output }), false);
  assert.equal(fs.existsSync(output), false);
});

test('an empty signature is rejected instead of advertising an unusable update', t => {
  const root = fixture(t);
  signed(root, 'linux', 'diaspor-vfs-updater.AppImage', '  \n');
  assert.throws(() => createManifest(options(root)), /signature/i);
});

test('partial successful platforms are valid, but macOS DMGs are not updater payloads', t => {
  const root = fixture(t);
  signed(root, 'macos', 'diaspor-vfs.dmg');
  signed(root, 'linux', 'diaspor-vfs-updater.AppImage');
  assert.deepEqual(Object.keys(createManifest(options(root)).platforms), ['linux-x86_64']);
});

test('collect copies signed macOS archive bytes and leaves normal installer untouched', t => {
  const root = fixture(t), bundles = path.join(root, 'bundle'), output = path.join(root, 'output');
  fs.mkdirSync(bundles);
  fs.mkdirSync(output);
  fs.writeFileSync(path.join(bundles, 'Diaspor.app.tar.gz'), 'archive');
  fs.writeFileSync(path.join(bundles, 'Diaspor.app.tar.gz.sig'), 'signature');
  fs.writeFileSync(path.join(output, 'diaspor-vfs.dmg'), 'installer');
  collectUpdaterArtifacts({ platform: 'macos', bundleDir: bundles, outputDir: output });
  assert.equal(fs.readFileSync(path.join(output, 'diaspor-vfs.app.tar.gz'), 'utf8'), 'archive');
  assert.equal(fs.readFileSync(path.join(output, 'diaspor-vfs.app.tar.gz.sig'), 'utf8'), 'signature');
  assert.equal(fs.readFileSync(path.join(output, 'diaspor-vfs.dmg'), 'utf8'), 'installer');
});

test('collect rejects missing or ambiguous signed payloads', t => {
  const root = fixture(t);
  assert.throws(() => collectUpdaterArtifacts({ platform: 'windows', bundleDir: root, outputDir: path.join(root, 'out') }), /exactly one/i);
  for (const name of ['one.msi', 'two.msi']) {
    fs.writeFileSync(path.join(root, name), 'installer');
    fs.writeFileSync(path.join(root, name + '.sig'), 'signature');
  }
  assert.throws(() => collectUpdaterArtifacts({ platform: 'windows', bundleDir: root, outputDir: path.join(root, 'out') }), /exactly one/i);
});

test('release metadata must not fall back to a stale version or accept unsafe URL paths', t => {
  const root = fixture(t);
  signed(root, 'linux', 'diaspor-vfs-updater.AppImage');
  for (const changes of [{ version: undefined }, { version: 'invalid' }, { tag: '../release' }, { repository: 'owner/repo/other' }]) {
    assert.throws(() => createManifest({ ...options(root), ...changes }), /version|tag|repository/i);
  }
});



test('CLI collects Windows/Linux signatures and emits a usable feed end to end', t => {
  const root = fixture(t), artifacts = path.join(root, 'artifacts');
  const script = fileURLToPath(new URL('./generate-update-manifest.js', import.meta.url));
  for (const [platform, input, output] of [
    ['windows', 'Diaspor_2.0.7_x64.msi', 'diaspor-vfs-updater.msi'],
    ['linux', 'Diaspor_2.0.7_amd64.AppImage', 'diaspor-vfs-updater.AppImage'],
  ]) {
    const bundle = path.join(root, platform), destination = path.join(artifacts, platform);
    fs.mkdirSync(bundle);
    fs.mkdirSync(destination, { recursive: true });
    fs.writeFileSync(path.join(bundle, input), 'updater bytes');
    fs.writeFileSync(path.join(bundle, input + '.sig'), platform + '-signature');
    fs.writeFileSync(path.join(destination, input.endsWith('.msi') ? 'diaspor-vfs.msi' : 'diaspor-vfs.AppImage'), 'original installer');
    const collect = spawnSync(process.execPath, [script, 'collect', '--platform', platform, '--bundle-dir', bundle, '--output-dir', destination]);
    assert.equal(collect.status, 0, collect.stderr.toString());
    assert.equal(fs.readFileSync(path.join(destination, output), 'utf8'), 'updater bytes');
    assert.equal(fs.readFileSync(path.join(destination, input.endsWith('.msi') ? 'diaspor-vfs.msi' : 'diaspor-vfs.AppImage'), 'utf8'), 'original installer');
  }
  const output = path.join(root, 'latest.json');
  const manifest = spawnSync(process.execPath, [script, 'manifest', '--artifacts', artifacts, '--output', output, '--version', '2.0.7', '--tag', 'v0.1.0-alpha.18', '--repository', 'stonyp90/diaspor']);
  assert.equal(manifest.status, 0, manifest.stderr.toString());
  const result = JSON.parse(fs.readFileSync(output, 'utf8'));
  assert.equal(result.platforms['windows-x86_64'].signature, 'windows-signature');
  assert.match(result.platforms['linux-x86_64'].url, /diaspor-vfs-updater.AppImage$/);
  assert.equal(spawnSync(process.execPath, [script, 'collect', '--platform']).status, 1);
});
