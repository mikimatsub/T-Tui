import test from 'node:test';
import assert from 'node:assert/strict';
import { binaryPath } from '../npm/bin/ttui.mjs';

test('launcher resolves only supported bundled binaries', () => {
  assert.match(binaryPath('win32', 'x64').replaceAll('\\', '/'), /\/win32-x64\/ttui\.exe$/);
  assert.match(binaryPath('linux', 'x64').replaceAll('\\', '/'), /\/linux-x64\/ttui$/);
  for (const [platform, arch] of [['darwin', 'x64'], ['linux', 'arm64'], ['win32', '../x64']]) {
    assert.throws(() => binaryPath(platform, arch), /supports Linux and Windows x64/);
  }
});
