#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const path = require('node:path');

const executable = path.join(
  __dirname,
  '..',
  'native',
  process.platform === 'win32' ? 'gitsama.exe' : 'gitsama',
);
const result = spawnSync(executable, process.argv.slice(2), { stdio: 'inherit' });

if (result.error) {
  process.stderr.write(`GitSama could not start its native binary: ${result.error.message}\n`);
  process.exit(1);
}

process.exit(result.status ?? 1);
