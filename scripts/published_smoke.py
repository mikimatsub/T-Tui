"""Verify unauthenticated release downloads and run only attested executables."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from urllib.request import urlopen

from release import TARGETS, bundle, version


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--commit', required=True)
    args = parser.parse_args()
    v = version()
    tag = f'v{v}'
    repo = 'mikimatsub/T-Tui'
    npm = shutil.which('npm.cmd' if os.name == 'nt' else 'npm')
    assert npm, 'npm is required for the installed-package check'
    with tempfile.TemporaryDirectory(prefix='ttui-published-') as temp:
        root = Path(temp)
        archives = [f'ttui-{v}-{target}.{ "zip" if platform == "win32-x64" else "tar.gz" }'
                    for platform, target in TARGETS.items()]
        tarball = f'mikimatsub-ttui-{v}.tgz'
        for name in archives + [name + '.sha256' for name in archives] + [tarball]:
            # Public asset requests deliberately contain no Authorization header.
            url = f'https://github.com/{repo}/releases/download/{tag}/{name}'
            with urlopen(url, timeout=60) as response, (root / name).open('wb') as output:
                shutil.copyfileobj(response, output)
        for name in archives + [tarball]:
            subprocess.run(['gh', 'attestation', 'verify', str(root / name), '--repo', repo,
                            '--signer-workflow', f'{repo}/.github/workflows/release.yml',
                            '--source-ref', f'refs/tags/{tag}', '--source-digest', args.commit,
                            '--deny-self-hosted-runners'], check=True)
        # This checks both published archive hashes and reads only exact regular members.
        stage = root / 'stage'
        bundle(root, stage)
        platform = 'win32-x64' if os.name == 'nt' else 'linux-x64'
        executable = 'ttui.exe' if os.name == 'nt' else 'ttui'
        binary = stage / 'bin' / platform / executable
        install = root / 'npm-install'
        subprocess.run([npm, 'install', '--prefix', str(install), '--ignore-scripts',
                        '--no-audit', '--no-fund', str(root / tarball)], check=True)
        shim = install / 'node_modules/.bin' / ('ttui.cmd' if os.name == 'nt' else 'ttui')
        env = dict(os.environ, TTUI_DATA_DIR=str(root / 'data'))
        for command in [binary, shim]:
            def run(*arguments):
                return subprocess.check_output([str(command), *arguments], env=env, text=True).strip()
            assert run('--version') == f'ttui {v}'
            assert 'No account data read' in run('--doctor')
            assert 'Read-only checks complete' in run('--mock', '--check')
            invalid = subprocess.run([str(command), '--invalid-option'], env=env,
                                     text=True, capture_output=True)
            assert invalid.returncode != 0 and 'Unknown argument' in invalid.stderr
    print(f'Public archives and npm tarball verified on {platform}: {tag}')


if __name__ == '__main__':
    main()
