'use strict';

const { spawnSync } = require('node:child_process');
const { existsSync, realpathSync } = require('node:fs');
const path = require('node:path');

function npmCli() {
  const nodeDirectory = path.dirname(process.execPath);
  const candidates = [
    path.join(nodeDirectory, 'node_modules/npm/bin/npm-cli.js'),
    path.join(nodeDirectory, '../lib/node_modules/npm/bin/npm-cli.js'),
  ];
  // Never resolve a command through the project directory or a relative PATH entry.
  for (const directory of (process.env.PATH || '').split(path.delimiter)) {
    if (!path.isAbsolute(directory)) continue;
    candidates.push(path.join(directory, 'node_modules/npm/bin/npm-cli.js'));
    const executable = path.join(directory, 'npm');
    if (existsSync(executable)) {
      const resolved = realpathSync(executable);
      if (path.basename(resolved) === 'npm-cli.js') candidates.push(resolved);
    }
  }
  const cli = candidates.find((candidate) => existsSync(candidate));
  if (!cli) throw new Error('Could not locate npm beside Node.js or on the absolute PATH. Reinstall with npm install --global gitsama@latest.');
  return realpathSync(cli);
}

function npm(args, packageRoot, options = {}) {
  return spawnSync(process.execPath, [npmCli(), ...args], {
    ...options,
    cwd: packageRoot,
    windowsHide: true,
  });
}

function samePath(first, second) {
  const a = realpathSync(first);
  const b = realpathSync(second);
  return process.platform === 'win32' ? a.toLowerCase() === b.toLowerCase() : a === b;
}

function upgrade({ version }, packageRoot) {
  if (typeof version !== 'string' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
    throw new Error('The native binary requested an invalid version.');
  }
  const globalRoot = npm(['root', '--global'], packageRoot, { encoding: 'utf8', timeout: 30000 });
  if (globalRoot.error) throw globalRoot.error;
  if (globalRoot.status !== 0) throw new Error('Could not find the global npm installation.');
  const globalPackage = path.join(globalRoot.stdout.trim(), 'gitsama');
  if (!existsSync(globalPackage) || !samePath(globalPackage, packageRoot)) {
    throw new Error(`This is a local installation; run npm install gitsama@${version} in its project.`);
  }
  process.stdout.write(`Installing gitsama@${version} with npm...\n`);
  // The old native binary has exited, so npm can replace it on Windows. Pin the
  // advertised version so an unpublished release cannot silently install an older one.
  const result = npm(['install', '--global', `gitsama@${version}`, '--registry', 'https://registry.npmjs.org', '--ignore-scripts=false'], packageRoot, {
    stdio: 'inherit',
    env: { ...process.env, GITSAMA_NONINTERACTIVE: '1' },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`npm exited with code ${result.status ?? 1}. Your voice packs and settings are preserved.`);
  process.stdout.write(`Updated GitSama to ${version}. Your voice packs and settings are preserved.\n`);
  return 0;
}

module.exports = { upgrade };
