#!/usr/bin/env python3
"""Offline packaging only. Never publishes or downloads executable code."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {'linux-x64': 'x86_64-unknown-linux-gnu', 'win32-x64': 'x86_64-pc-windows-msvc'}

def version():
    rust = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    package = json.loads((ROOT / 'npm/package.json').read_text())
    assert rust == package['version'], 'Cargo and npm versions differ'
    assert re.fullmatch(r'\d+\.\d+\.\d+(?:-rc\.\d+)?', rust), 'Unexpected release version'
    assert not package.get('scripts') and not package.get('dependencies'), 'Launcher must not add install scripts or dependencies'
    return rust

def digest(path):
    return hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()

def native(platform, binary, output):
    v = version()
    binary = binary.resolve()
    assert binary.is_file(), f'Missing native binary: {binary}'
    actual = subprocess.check_output([str(binary), '--version'], text=True).strip()
    assert actual == f'ttui {v}', f'Binary version mismatch: {actual}'
    output.mkdir(parents=True, exist_ok=True)
    name = f'ttui-{v}-{TARGETS[platform]}'
    members = {('ttui.exe' if platform == 'win32-x64' else 'ttui'): binary,
               'LICENSE': ROOT / 'LICENSE', 'README.md': ROOT / 'README.md'}
    if platform == 'win32-x64':
        archive = output / f'{name}.zip'
        with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as writer:
            for member, path in members.items():
                writer.write(path, member)
    else:
        archive = output / f'{name}.tar.gz'
        with tarfile.open(archive, 'w:gz') as writer:
            for member, path in members.items():
                info = writer.gettarinfo(str(path), member)
                info.uid = info.gid = 0
                info.uname = info.gname = ''
                info.mode = 0o755 if member == 'ttui' else 0o644
                with path.open('rb') as source:
                    writer.addfile(info, source)
    (output / f'{archive.name}.sha256').write_text(f'{digest(archive)}  {archive.name}\n')
    print(archive)

def bundle(artifacts, output):
    v = version()
    output.mkdir(parents=True, exist_ok=False)
    shutil.copytree(ROOT / 'npm/bin', output / 'bin')
    shutil.copy2(ROOT / 'npm/package.json', output / 'package.json')
    for filename in ['LICENSE', 'README.md']:
        shutil.copy2(ROOT / filename, output / filename)
    hashes = {}
    for platform, target in TARGETS.items():
        ext = 'zip' if platform == 'win32-x64' else 'tar.gz'
        path = artifacts / f'ttui-{v}-{target}.{ext}'
        expected = path.with_name(path.name + '.sha256').read_text().split()[0]
        assert digest(path) == expected, f'Archive checksum mismatch: {path.name}'
        member = 'ttui.exe' if platform == 'win32-x64' else 'ttui'
        # Read only the exact executable member; never extract archive paths.
        if ext == 'zip':
            with zipfile.ZipFile(path) as archive:
                data = archive.read(member)
        else:
            with tarfile.open(path) as archive:
                data = archive.extractfile(member).read()
        destination = output / 'bin' / platform / member
        destination.parent.mkdir(parents=True)
        destination.write_bytes(data)
        destination.chmod(0o755)
        hashes[f'bin/{platform}/{member}'] = hashlib.sha256(data).hexdigest()
    (output / 'checksums.json').write_text(json.dumps(hashes, indent=2) + '\n')
    (output / 'bin/ttui.mjs').chmod(0o755)
    print(output)

def winget(artifacts, output):
    v = version()
    assert '-' not in v, 'WinGet submission is reserved for stable releases'
    archive = artifacts / f'ttui-{v}-{TARGETS["win32-x64"]}.zip'
    sha = digest(archive).upper()
    manifest = f'''PackageIdentifier: mikimatsub.TTUI
PackageVersion: {v}
PackageLocale: en-US
Publisher: mikimatsub
PackageName: T-TUI
License: MIT
LicenseUrl: https://github.com/mikimatsub/T-Tui/blob/v{v}/LICENSE
ShortDescription: Terminal client for Tinder
PackageUrl: https://github.com/mikimatsub/T-Tui
Installers:
- Architecture: x64
  InstallerType: zip
  NestedInstallerType: portable
  NestedInstallerFiles:
  - RelativeFilePath: ttui.exe
    PortableCommandAlias: ttui
  InstallerUrl: https://github.com/mikimatsub/T-Tui/releases/download/v{v}/{archive.name}
  InstallerSha256: {sha}
ManifestType: singleton
ManifestVersion: 1.12.0
'''
    output.mkdir(parents=True, exist_ok=True)
    (output / 'mikimatsub.TTUI.yaml').write_text(manifest)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    check = sub.add_parser('check')
    check.add_argument('--tag')
    pack = sub.add_parser('native')
    pack.add_argument('--platform', choices=TARGETS, required=True)
    pack.add_argument('--binary', type=Path, required=True)
    pack.add_argument('--output', type=Path, default=ROOT / 'dist')
    for command in ['bundle', 'winget']:
        child = sub.add_parser(command)
        child.add_argument('--artifacts', type=Path, required=True)
        child.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'check':
        v = version()
        if args.tag:
            assert args.tag == f'v{v}', 'Tag does not match source version'
        print(v)
    elif args.command == 'native': native(args.platform, args.binary, args.output)
    elif args.command == 'bundle': bundle(args.artifacts, args.output)
    elif args.command == 'winget': winget(args.artifacts, args.output)

if __name__ == '__main__':
    main()
