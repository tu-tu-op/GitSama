'use strict';

const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const { lstat, mkdir, mkdtemp, realpath, rename, rm, writeFile } = require('node:fs/promises');
const path = require('node:path');
const { version } = require('../package.json');

const repository = 'tu-tu-op/GitSama';
const maxBinaryBytes = 100 * 1024 * 1024;

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

function parseSha256(text, expectedAsset) {
  const match = text.trim().match(/^([a-f\d]{64})(?:[ \t]+\*?([^\r\n]+))?$/i);
  if (!match) throw new Error('The release checksum file is invalid.');
  if (expectedAsset && match[2] !== expectedAsset) throw new Error('The release checksum names an unexpected asset.');
  return match[1].toLowerCase();
}

function verifySha256(binary, checksumText, expectedAsset) {
  const expected = parseSha256(checksumText, expectedAsset);
  const actual = createHash('sha256').update(binary).digest('hex');
  if (actual !== expected) throw new Error('The downloaded GitSama binary failed its SHA-256 check.');
}

async function fetchBuffer(url, limit = maxBinaryBytes) {
  if (new URL(url).protocol !== 'https:') throw new Error('Release downloads require HTTPS.');
  const response = await fetch(url, { signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`Download failed (${response.status}): ${url}`);
  if (new URL(response.url).protocol !== 'https:') throw new Error('The release download redirected away from HTTPS.');
  const length = response.headers.get('content-length');
  if (length !== null && (!/^\d+$/.test(length) || Number(length) > limit)) {
    await response.body?.cancel();
    throw new Error('Release download exceeds its size limit.');
  }
  const chunks = [];
  let bytes = 0;
  if (!response.body) throw new Error('The release download has no response body.');
  for await (const chunk of response.body) {
    bytes += chunk.length;
    if (bytes > limit) throw new Error('Release download exceeds its size limit.');
    chunks.push(Buffer.from(chunk));
  }
  return Buffer.concat(chunks, bytes);
}

async function install() {
  const target = releaseAsset();
  const base = `https://github.com/${repository}/releases/download/v${version}/`;
  const [binary, checksumFile] = await Promise.all([
    fetchBuffer(base + target.asset),
    fetchBuffer(base + `${target.asset}.sha256`, 1024),
  ]);
  verifySha256(binary, checksumFile.toString('utf8'), target.asset);

  const nativeDirectory = path.join(__dirname, '..', 'native');
  const executablePath = path.join(nativeDirectory, target.executable);
  await mkdir(nativeDirectory, { recursive: true });
  if ((await lstat(nativeDirectory)).isSymbolicLink()) throw new Error('The native directory must not be a symlink.');
  const staging = await mkdtemp(path.join(await realpath(nativeDirectory), '.download-'));
  try {
    const temporaryPath = path.join(staging, target.executable);
    await writeFile(temporaryPath, binary, { mode: 0o755, flag: 'wx' });
    await rename(temporaryPath, executablePath);
  } finally {
    await rm(staging, { recursive: true, force: true });
  }

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

module.exports = { fetchBuffer, parseSha256, releaseAsset, verifySha256 };
