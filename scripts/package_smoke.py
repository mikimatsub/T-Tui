"""Pack, inspect, install locally without scripts, and execute the actual npm shim."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tarfile
from release import ROOT, digest, version

def run(args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()

def main():
    npm = shutil.which('npm.cmd' if os.name == 'nt' else 'npm')
    assert npm, 'npm unavailable'
    packed = json.loads(run([npm, 'pack', '--json', '--ignore-scripts', '--pack-destination', str(ROOT / 'dist')], cwd=ROOT / 'target/npm-stage'))[0]
    names = {entry['path'] for entry in packed['files']}
    expected = {'package.json', 'LICENSE', 'CROSSTERM-LICENSE', 'README.md', 'checksums.json', 'bin/ttui.mjs', 'bin/linux-x64/ttui', 'bin/win32-x64/ttui.exe'}
    assert names == expected, f'Unexpected package contents: {names ^ expected}'
    tarball = ROOT / 'dist' / packed['filename']
    with tarfile.open(tarball) as archive:
        import hashlib
        hashes = json.load(archive.extractfile('package/checksums.json'))
        for path, sha in hashes.items():
            assert hashlib.sha256(archive.extractfile('package/' + path).read()).hexdigest() == sha
    with tempfile.TemporaryDirectory(prefix='ttui-package-') as temp:
        run([npm, 'install', '--prefix', temp, '--ignore-scripts', '--no-audit', '--no-fund', str(tarball)])
        shim = Path(temp) / 'node_modules/.bin' / ('ttui.cmd' if os.name == 'nt' else 'ttui')
        env = dict(os.environ, TTUI_DATA_DIR=str(Path(temp) / 'data'))
        assert run([str(shim), '--version'], env=env) == f'ttui {version()}'
        assert 'No account data read' in run([str(shim), '--doctor'], env=env)
        assert 'Read-only checks complete' in run([str(shim), '--mock', '--check'], env=env)
        failed = subprocess.run([str(shim), '--invalid-option'], env=env, capture_output=True, text=True)
        assert failed.returncode != 0 and 'Unknown argument' in failed.stderr
    print(f'Package contents, checksums, installed shim, offline checks and error exit verified: {tarball.name}')

if __name__ == '__main__':
    main()
