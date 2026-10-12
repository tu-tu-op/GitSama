'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { upgrade } = require('../scripts/upgrade');

test('rejects versions that could become npm options or shell commands', () => {
  for (const version of ['--help', '1.2.3 & echo bad', '1.2.3\n', '01.2.3', null]) {
    assert.throws(() => upgrade({ version }, __dirname), /invalid version/);
  }
});

test('an update from an untrusted repository never invokes its npm wrapper', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'gitsama-upgrade-test-'));
  try {
    const marker = path.join(directory, 'executed');
    fs.writeFileSync(path.join(directory, 'npm.cmd'), `@echo off\r\necho bad>"${marker}"\r\nexit /b 1\r\n`);
    fs.writeFileSync(path.join(directory, 'npm'), `#!/bin/sh\nprintf bad > '${marker.replaceAll("'", "'\\''")}'\nexit 1\n`, { mode: 0o755 });
    const script = `const { upgrade } = require(${JSON.stringify(path.resolve(__dirname, '../scripts/upgrade'))});
      try { upgrade({ version: '0.1.2' }, ${JSON.stringify(directory)}); process.exit(2); }
      catch (error) { if (!error.message.includes('local installation')) throw error; }`;
    const result = spawnSync(process.execPath, ['-e', script], {
      cwd: directory,
      env: { ...process.env, PATH: `.${path.delimiter}${process.env.PATH || ''}` },
      encoding: 'utf8',
      timeout: 35000,
      windowsHide: true,
    });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(fs.existsSync(marker), false);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
