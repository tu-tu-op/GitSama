'use strict';

const { spawnSync } = require('node:child_process');
const { existsSync, realpathSync } = require('node:fs');
const path = require('node:path');

function npm(args, options = {}) {
  // These arguments are fixed below, with the version restricted to digits and
  // dots. Windows needs cmd to launch npm.cmd; no user paths enter its command.
  return process.platform === 'win32'
    ? spawnSync(process.env.ComSpec || 'cmd.exe', ['/d', '/s', '/c', `npm ${args.join(' ')}`], options)
    : spawnSync('npm', args, options);
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
  const globalRoot = npm(['root', '--global'], { encoding: 'utf8', timeout: 30000 });
  if (globalRoot.error) throw globalRoot.error;
  if (globalRoot.status !== 0) throw new Error('Could not find the global npm installation.');
  const globalPackage = path.join(globalRoot.stdout.trim(), 'gitsama');
  if (!existsSync(globalPackage) || !samePath(globalPackage, packageRoot)) {
    throw new Error(`This is a local installation; run npm install gitsama@${version} in its project.`);
  }
  process.stdout.write(`Installing gitsama@${version} with npm...\n`);
  // The old native binary has exited, so npm can replace it on Windows. Pin the
  // advertised version so an unpublished release cannot silently install an older one.
  const result = npm(['install', '--global', `gitsama@${version}`, '--registry', 'https://registry.npmjs.org', '--ignore-scripts=false'], {
    stdio: 'inherit',
    env: { ...process.env, GITSAMA_NONINTERACTIVE: '1' },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`npm exited with code ${result.status ?? 1}. Your voice packs and settings are preserved.`);
  process.stdout.write(`Updated GitSama to ${version}. Your voice packs and settings are preserved.\n`);
  return 0;
}

module.exports = { upgrade };
