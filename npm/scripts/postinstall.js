'use strict';

const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const { mkdir, rename, writeFile } = require('node:fs/promises');
const path = require('node:path');
const { version } = require('../package.json');

const repository = 'tu-tu-op/GitSama';

function releaseAsset(platform = process.platform, arch = process.arch) {
  const os = { win32: 'windows', darwin: 'macos', linux: 'linux' }[platform];
  const architecture = { x64: 'x86_64', arm64: 'aarch64' }[arch];
  if (!os || !architecture || (platform === 'win32' && arch !== 'x64') || (platform === 'linux' && arch !== 'x64')) {
    throw new Error(`No prebuilt GitSama release is available for ${platform}/${arch}.`);
  }

  const extension = platform === 'win32' ? '.exe' : '';
  const asset = `gitsama-v${version}-${os}-${architecture}${extension}`;
  return { asset, executable: `gitsama${extension}` };
}

function parseSha256(text) {
  const match = text.trim().match(/^([a-f\d]{64})(?:\s+\*?.+)?$/i);
  if (!match) throw new Error('The release checksum file is invalid.');
  return match[1].toLowerCase();
}

function verifySha256(binary, checksumText) {
  const expected = parseSha256(checksumText);
  const actual = createHash('sha256').update(binary).digest('hex');
  if (actual !== expected) throw new Error('The downloaded GitSama binary failed its SHA-256 check.');
}

async function fetchBuffer(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`Download failed (${response.status}): ${url}`);
  return Buffer.from(await response.arrayBuffer());
}

async function install() {
  const target = releaseAsset();
  const base = `https://github.com/${repository}/releases/download/v${version}/`;
  const [binary, checksumFile] = await Promise.all([
    fetchBuffer(base + target.asset),
    fetchBuffer(base + `${target.asset}.sha256`),
  ]);
  verifySha256(binary, checksumFile.toString('utf8'));

  const nativeDirectory = path.join(__dirname, '..', 'native');
  const executablePath = path.join(nativeDirectory, target.executable);
  const temporaryPath = `${executablePath}.download`;
  await mkdir(nativeDirectory, { recursive: true });
  await writeFile(temporaryPath, binary, { mode: 0o755 });
  await rename(temporaryPath, executablePath);

  if (process.platform !== 'win32') {
    const fs = require('node:fs/promises');
    await fs.chmod(executablePath, 0o755);
  }

  const setup = spawnSync(executablePath, ['setup'], {
    stdio: 'inherit',
    env: { ...process.env, GITSAMA_NONINTERACTIVE: '1' },
  });
  if (setup.error) throw new Error(`GitSama setup could not start: ${setup.error.message}`);
  if (setup.status !== 0) throw new Error(`GitSama setup failed with exit code ${setup.status ?? 1}.`);
}

if (require.main === module) {
  install().catch((error) => {
    process.stderr.write(`GitSama npm installation failed: ${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = { parseSha256, releaseAsset, verifySha256 };
