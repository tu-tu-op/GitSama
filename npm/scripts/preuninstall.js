'use strict';

const { spawnSync } = require('node:child_process');
const { existsSync } = require('node:fs');
const path = require('node:path');

const executable = path.join(
  __dirname,
  '..',
  'native',
  process.platform === 'win32' ? 'gitsama.exe' : 'gitsama',
);

if (existsSync(executable)) {
  const result = spawnSync(executable, ['uninstall', '--keep-data'], {
    stdio: 'inherit',
    env: { ...process.env, GITSAMA_NONINTERACTIVE: '1' },
  });
  if (result.error) {
    process.stderr.write(`GitSama hooks could not be removed: ${result.error.message}\n`);
    process.exit(1);
  }
  if (result.status !== 0) process.exit(result.status ?? 1);
}
