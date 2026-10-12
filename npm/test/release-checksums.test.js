'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { version } = require('../package.json');
const { assets, loadChecksums, prepare, validateManifest } = require('../scripts/release-checksums');
const { verifySha256 } = require('../scripts/postinstall');

test('release preparation pins all platform binaries and rejects substituted bytes', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'gitsama-release-test-'));
  try {
    for (const asset of assets) {
      const bytes = Buffer.from(`original ${asset}`);
      fs.writeFileSync(path.join(directory, asset), bytes);
      fs.writeFileSync(path.join(directory, `${asset}.sha256`), `${createHash('sha256').update(bytes).digest('hex')}  ${asset}\n`);
    }
    const output = path.join(directory, 'manifest.json');
    await prepare(directory, output);
    const manifest = loadChecksums(output);
    assert.equal(manifest.version, version);
    for (const asset of assets) verifySha256(fs.readFileSync(path.join(directory, asset)), manifest.assets[asset]);
    const changed = Buffer.from('substituted executable');
    const forgedRemoteChecksum = createHash('sha256').update(changed).digest('hex');
    verifySha256(changed, forgedRemoteChecksum);
    assert.throws(() => verifySha256(changed, manifest.assets[assets[0]]), /SHA-256/);
    fs.writeFileSync(path.join(directory, assets[0]), changed);
    await assert.rejects(prepare(directory, output), /mismatch/);
    assert.throws(() => validateManifest({ ...manifest, version: '9.9.9' }), /another version/);
    assert.throws(() => validateManifest({ version, assets: {} }), /invalid/);
    assert.throws(() => validateManifest({ ...manifest, assets: { ...manifest.assets, [assets[0]]: [manifest.assets[assets[0]]] } }), /invalid/);
    fs.writeFileSync(path.join(directory, `${assets[0]}.sha256`), Buffer.alloc(1025));
    await assert.rejects(prepare(directory, output), /Invalid release checksum file/);
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});
