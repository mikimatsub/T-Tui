#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { realpathSync } from 'node:fs';

export function binaryPath(platform, arch) {
  if (arch !== 'x64' || !['linux', 'win32'].includes(platform)) {
    throw new Error(`T-TUI supports Linux and Windows x64; received ${platform}/${arch}. See GitHub Releases or build from source.`);
  }
  return fileURLToPath(new URL(`${platform}-x64/ttui${platform === 'win32' ? '.exe' : ''}`, import.meta.url));
}

if (process.argv[1] && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const child = spawn(binaryPath(process.platform, process.arch), process.argv.slice(2), { stdio: 'inherit', shell: false });
    const signals = ['SIGINT', 'SIGTERM'];
    const handlers = signals.map(signal => () => child.kill(signal));
    signals.forEach((signal, index) => process.on(signal, handlers[index]));
    child.on('error', error => {
      console.error(`Could not launch T-TUI (${error.code ?? 'unknown error'}). Reinstall the package or use the native GitHub release.`);
      process.exitCode = 1;
    });
    child.on('exit', (code, signal) => {
      signals.forEach((name, index) => process.removeListener(name, handlers[index]));
      process.exitCode = code ?? (signal === 'SIGINT' ? 130 : 143);
    });
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
