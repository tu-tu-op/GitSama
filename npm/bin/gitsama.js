#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const { randomUUID } = require('node:crypto');
const { readFileSync, rmSync } = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { upgrade } = require('../scripts/upgrade');

const executable = path.join(
  __dirname,
  '..',
  'native',
  process.platform === 'win32' ? 'gitsama.exe' : 'gitsama',
);
const requestPath = path.join(os.tmpdir(), `gitsama-upgrade-${randomUUID()}.json`);
const result = spawnSync(executable, process.argv.slice(2), {
  stdio: 'inherit',
  env: { ...process.env, GITSAMA_NPM_UPGRADE_FILE: requestPath },
});

if (result.error) {
  process.stderr.write(`GitSama could not start its native binary: ${result.error.message}\n`);
  process.exit(1);
}

let status = result.status ?? 1;
let automatic = false;
try {
  if (status === 75) {
    const request = JSON.parse(readFileSync(requestPath, 'utf8'));
    automatic = request.automatic === true;
    status = upgrade(request, path.resolve(__dirname, '..'));
  }
} catch (error) {
  process.stderr.write(`GitSama npm update failed: ${error.message}\nFor a global installation, retry with npm install --global gitsama@latest.\n`);
  status = automatic ? 0 : 1;
} finally {
  rmSync(requestPath, { force: true });
}
process.exit(status);
