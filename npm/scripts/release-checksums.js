'use strict';

const { createHash } = require('node:crypto');
const { createReadStream, lstatSync, readFileSync } = require('node:fs');
const { readFile, writeFile } = require('node:fs/promises');
const path = require('node:path');
const { version } = require('../package.json');

const manifestPath = path.join(__dirname, '..', 'release-checksums.json');
const assets = ['windows-x86_64.exe', 'linux-x86_64', 'macos-x86_64', 'macos-aarch64']
  .map((target) => `gitsama-v${version}-${target}`);

function validateManifest(manifest) {
  if (manifest?.version !== version || !manifest.assets || Object.keys(manifest.assets).length !== assets.length
      || assets.some((asset) => !Object.hasOwn(manifest.assets, asset) || !/^[a-f\d]{64}$/i.test(manifest.assets[asset]))) {
    throw new Error('The packaged release checksum manifest is invalid or belongs to another version.');
  }
  return manifest;
}

function loadChecksums(file = manifestPath) {
  try {
    const metadata = lstatSync(file);
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 16384) throw new Error('invalid manifest file');
    return validateManifest(JSON.parse(readFileSync(file, 'utf8')));
  } catch (error) {
    throw new Error('Missing or invalid packaged release checksums. Maintainers must run npm run prepare-release before publishing.', { cause: error });
  }
}

async function prepare(directory, output = manifestPath) {
  // Load the existing download/checksum helpers only when preparing a release.
  const { fetchBuffer, parseSha256 } = require('./postinstall');
  const checksums = {};
  for (const asset of assets) {
    const checksum = directory
      ? await readFile(path.join(directory, `${asset}.sha256`))
      : await fetchBuffer(`https://github.com/tu-tu-op/GitSama/releases/download/v${version}/${asset}.sha256`, 1024);
    if (checksum.length > 1024) throw new Error('Release checksum exceeds its size limit.');
    checksums[asset] = parseSha256(checksum.toString('utf8'), asset);
    if (directory) {
      const file = path.join(directory, asset);
      const metadata = lstatSync(file);
      if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 100 * 1024 * 1024) throw new Error('Invalid release binary.');
      const hash = createHash('sha256');
      for await (const chunk of createReadStream(file)) hash.update(chunk);
      if (hash.digest('hex') !== checksums[asset]) throw new Error(`Release binary checksum mismatch: ${asset}`);
    }
  }
  const manifest = validateManifest({ version, assets: checksums });
  await writeFile(output, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600 });
  return manifest;
}

module.exports = { assets, loadChecksums, prepare, validateManifest };

if (require.main === module) {
  if (process.argv[2] === '--check') {
    try { loadChecksums(); } catch (error) { process.stderr.write(`${error.message}\n`); process.exitCode = 1; }
  } else {
    prepare(process.argv[2]).then(() => process.stdout.write(`Prepared pinned binary checksums for ${version}.\n`))
      .catch((error) => { process.stderr.write(`${error.message}\n`); process.exitCode = 1; });
  }
}
