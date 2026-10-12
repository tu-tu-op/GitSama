'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { version: npmVersion } = require('../package.json');
const { fetchBuffer, parseSha256, releaseAsset, verifySha256 } = require('../scripts/postinstall');

test('npm and Rust package versions match', () => {
  const cargoToml = fs.readFileSync(path.join(__dirname, '..', '..', 'Cargo.toml'), 'utf8');
  assert.equal(cargoToml.match(/^version = "([^"]+)"/m)?.[1], npmVersion);
  const shellInstaller = fs.readFileSync(path.join(__dirname, '..', '..', 'install.sh'), 'utf8');
  assert.equal(shellInstaller.match(/VERSION="\$\{GITSAMA_VERSION:-([^}]+)\}"/)?.[1], npmVersion);
});

test('maps supported Windows and macOS builds to release assets', () => {
  assert.deepEqual(releaseAsset('win32', 'x64'), {
    asset: `gitsama-v${npmVersion}-windows-x86_64.exe`,
    executable: 'gitsama.exe',
  });
  assert.deepEqual(releaseAsset('darwin', 'arm64'), {
    asset: `gitsama-v${npmVersion}-macos-aarch64`,
    executable: 'gitsama',
  });
  assert.deepEqual(releaseAsset('darwin', 'x64'), {
    asset: `gitsama-v${npmVersion}-macos-x86_64`,
    executable: 'gitsama',
  });
});

test('maps Linux x64 and rejects unbuilt targets', () => {
  assert.equal(releaseAsset('linux', 'x64').asset, `gitsama-v${npmVersion}-linux-x86_64`);
  assert.throws(() => releaseAsset('linux', 'arm64'), /No prebuilt/);
  assert.throws(() => releaseAsset('win32', 'arm64'), /No prebuilt/);
});

test('accepts standard sha256sum output and rejects malformed checksums', () => {
  const digest = 'a'.repeat(64);
  assert.equal(parseSha256(`${digest}  gitsama`), digest);
  assert.throws(() => parseSha256('not-a-checksum'), /invalid/);
  assert.equal(parseSha256(`${digest} *expected`, 'expected'), digest);
  assert.throws(() => parseSha256(`${digest}  other`, 'expected'), /unexpected/);
  assert.throws(() => parseSha256(`${digest}  expected\nextra`), /invalid/);
});

test('bounds streamed downloads even when the server omits or lies about length', async () => {
  const original = global.fetch;
  try {
    for (const length of [null, '1']) {
      global.fetch = async () => ({
        ok: true, url: 'https://example.invalid/asset', headers: new Headers(length === null ? {} : { 'content-length': length }),
        body: new ReadableStream({ start(controller) { controller.enqueue(Buffer.alloc(5)); controller.enqueue(Buffer.alloc(5)); controller.close(); } }),
      });
      await assert.rejects(fetchBuffer('https://example.invalid/asset', 8), /size limit/);
    }
    global.fetch = async () => ({ ok: true, url: 'http://example.invalid/asset' });
    await assert.rejects(fetchBuffer('https://example.invalid/asset'), /HTTPS/);
    await assert.rejects(fetchBuffer('http://example.invalid/asset'), /HTTPS/);
  } finally { global.fetch = original; }
});

test('verifies the downloaded native binary before installing it', () => {
  const binary = Buffer.from('test binary');
  const digest = require('node:crypto').createHash('sha256').update(binary).digest('hex');
  verifySha256(binary, `${digest}  gitsama`);
  assert.throws(() => verifySha256(binary, `${'0'.repeat(64)}  gitsama`), /SHA-256/);
});
