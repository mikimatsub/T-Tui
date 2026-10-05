"""Offline release regressions; every filesystem operation stays in a temporary root."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import stat
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import warnings
import zipfile

import release


class ReleaseTests(unittest.TestCase):
    def test_winget_rejects_release_candidates_without_creating_manifest(self):
        with self.assertRaisesRegex(AssertionError, 'stable releases'):
            release.winget(self.artifacts, self.output)
        self.assertFalse(self.output.exists())

    def test_winget_emits_community_manifest_set_with_real_archive_hash(self):
        self.package['version'] = '1.0.0'
        self.write_versions(rust='1.0.0')
        archive = self.artifacts / 'ttui-1.0.0-x86_64-pc-windows-msvc.zip'
        archive.write_bytes(b'offline archive fixture')
        release.winget(self.artifacts, self.output)
        expected = {'mikimatsub.TTUI.yaml': 'version',
                    'mikimatsub.TTUI.locale.en-US.yaml': 'defaultLocale',
                    'mikimatsub.TTUI.installer.yaml': 'installer'}
        self.assertEqual({path.name for path in self.output.iterdir()}, set(expected))
        for name, kind in expected.items():
            content = (self.output / name).read_text(encoding='utf-8')
            self.assertTrue(content.startswith(f'# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.1.12.0.schema.json\n'))
            self.assertIn(f'ManifestType: {kind}\n', content)
            self.assertIn('PackageVersion: 1.0.0\n', content)
        installer = (self.output / 'mikimatsub.TTUI.installer.yaml').read_text()
        self.assertIn('InstallerSha256: ' + hashlib.sha256(archive.read_bytes()).hexdigest().upper(), installer)
        self.assertIn('https://github.com/mikimatsub/T-Tui/releases/download/v1.0.0/' + archive.name, installer)
        self.assertIn('RelativeFilePath: ttui.exe', installer)
        self.assertIn('PortableCommandAlias: ttui', installer)

    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix='ttui-release-test-')
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        root_patch = patch.object(release, 'ROOT', self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        output_patch = contextlib.redirect_stdout(io.StringIO())
        output_patch.__enter__()
        self.addCleanup(output_patch.__exit__, None, None, None)
        self.package = {'name': '@mikimatsub/ttui', 'version': '1.0.0-rc.1'}
        self.write_versions()
        self.write('npm/bin/ttui.mjs', b'// test launcher fixture\n')
        self.write('LICENSE', b'test license\n')
        self.write('README.md', b'test readme\n')
        self.write('vendor/crossterm/LICENSE', b'test vendored license\n')
        self.artifacts = self.root / 'artifacts'
        self.artifacts.mkdir()
        self.output = self.root / 'stage'
        self.binaries = {'linux-x64': b'linux fixture bytes', 'win32-x64': b'windows fixture bytes'}

    def write(self, name, data):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return path

    def write_versions(self, rust='1.0.0-rc.1'):
        self.write('Cargo.toml', f'[package]\nversion = "{rust}"\n'.encode())
        self.write('npm/package.json', json.dumps(self.package).encode())

    def archive(self, platform, entries=None):
        windows = platform == 'win32-x64'
        member = 'ttui.exe' if windows else 'ttui'
        extension = 'zip' if windows else 'tar.gz'
        path = self.artifacts / f'ttui-1.0.0-rc.1-{release.TARGETS[platform]}.{extension}'
        if entries is None:
            entries = [(member, self.binaries[platform], 'file'),
                       ('../outside.txt', b'must never be extracted', 'file'),
                       ('unrelated/ttui', b'wrong executable', 'file')]
        if windows:
            with warnings.catch_warnings():
                warnings.simplefilter('ignore', UserWarning)  # Deliberate duplicate-member fixture.
                with zipfile.ZipFile(path, 'w') as writer:
                    for name, data, kind in entries:
                        info = zipfile.ZipInfo(name)
                        info.create_system = 3
                        mode = stat.S_IFLNK if kind == 'link' else stat.S_IFREG
                        info.external_attr = (mode | 0o755) << 16
                        writer.writestr(info, data)
        else:
            with tarfile.open(path, 'w:gz') as writer:
                for name, data, kind in entries:
                    info = tarfile.TarInfo(name)
                    if kind == 'link':
                        info.type = tarfile.SYMTYPE
                        info.linkname = data.decode()
                        writer.addfile(info)
                    else:
                        info.size = len(data)
                        writer.addfile(info, io.BytesIO(data))
        path.with_name(path.name + '.sha256').write_text(
            f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n')
        return path

    def archives(self):
        return {platform: self.archive(platform) for platform in self.binaries}

    def test_version_mismatch_rejects_before_creating_stage(self):
        self.package['version'] = '1.0.0'
        self.write_versions()
        with self.assertRaisesRegex(AssertionError, 'Cargo and npm versions differ'):
            release.bundle(self.artifacts, self.output)
        self.assertFalse(self.output.exists())

    def test_version_validation_accepts_stable_and_rc_but_rejects_other_syntax(self):
        for version in ['1.0.0', '1.0.0-rc.1']:
            with self.subTest(version=version):
                self.package['version'] = version
                self.write_versions(version)
                self.assertEqual(release.version(), version)
        for version in ['v1.0.0', '1.0', '1.0.0-beta.1', '1.0.0-rc.1/extra']:
            with self.subTest(version=version):
                self.package['version'] = version
                self.write_versions(version)
                with self.assertRaisesRegex(AssertionError, 'Unexpected release version'):
                    release.version()

    def test_version_validation_rejects_install_scripts_and_dependencies(self):
        for key, value in [('scripts', {'postinstall': 'not-allowed'}),
                           ('dependencies', {'unexpected': '1.0.0'})]:
            with self.subTest(key=key):
                self.package[key] = value
                self.write_versions()
                with self.assertRaisesRegex(AssertionError, 'install scripts or dependencies'):
                    release.version()
                del self.package[key]

    def test_native_binary_version_mismatch_creates_no_archive(self):
        binary = self.write('build/ttui.exe', b'never executed')
        output = self.root / 'native-output'
        with patch.object(release.subprocess, 'check_output', return_value='ttui 0.3.0\n') as command:
            with self.assertRaisesRegex(AssertionError, 'Binary version mismatch'):
                release.native('win32-x64', binary, output)
        command.assert_called_once_with([str(binary.resolve()), '--version'], text=True)
        self.assertFalse(output.exists())

    def test_native_archives_include_executable_licenses_and_matching_checksum(self):
        binary = self.write('build/ttui', b'local archive fixture')
        for platform in self.binaries:
            with self.subTest(platform=platform):
                output = self.root / platform
                with patch.object(release.subprocess, 'check_output', return_value='ttui 1.0.0-rc.1\n'):
                    release.native(platform, binary, output)
                archive = next(path for path in output.iterdir() if not path.name.endswith('.sha256'))
                member = 'ttui.exe' if platform == 'win32-x64' else 'ttui'
                expected = {member, 'LICENSE', 'README.md', 'CROSSTERM-LICENSE'}
                if platform == 'win32-x64':
                    with zipfile.ZipFile(archive) as reader:
                        self.assertEqual(set(reader.namelist()), expected)
                        self.assertEqual(reader.read(member), b'local archive fixture')
                else:
                    with tarfile.open(archive) as reader:
                        self.assertEqual(set(reader.getnames()), expected)
                        self.assertEqual(reader.extractfile(member).read(), b'local archive fixture')
                        self.assertEqual(reader.getmember(member).mode, 0o755)
                        self.assertEqual(reader.getmember('LICENSE').mode, 0o644)
                checksum = archive.with_name(archive.name + '.sha256').read_text()
                self.assertEqual(checksum, f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n')

    def test_bundle_selects_only_exact_binaries_and_emits_both_checksums(self):
        self.archives()
        release.bundle(self.artifacts, self.output)
        expected_hashes = {}
        for platform, content in self.binaries.items():
            member = 'ttui.exe' if platform == 'win32-x64' else 'ttui'
            relative = f'bin/{platform}/{member}'
            self.assertEqual((self.output / relative).read_bytes(), content)
            expected_hashes[relative] = hashlib.sha256(content).hexdigest()
        self.assertEqual(json.loads((self.output / 'checksums.json').read_text()), expected_hashes)
        self.assertEqual((self.output / 'CROSSTERM-LICENSE').read_bytes(), b'test vendored license\n')
        self.assertEqual(json.loads((self.output / 'package.json').read_text())['version'], '1.0.0-rc.1')
        self.assertFalse((self.root / 'outside.txt').exists())
        self.assertFalse((self.output / 'unrelated').exists())

    def test_tampered_archive_checksum_rejects_binary_staging(self):
        for platform in self.binaries:
            with self.subTest(platform=platform):
                path = self.archives()[platform]
                path.write_bytes(path.read_bytes() + b'tampered after hashing')
                output = self.root / f'tampered-{platform}'
                member = 'ttui.exe' if platform == 'win32-x64' else 'ttui'
                with self.assertRaisesRegex(AssertionError, 'Archive checksum mismatch'):
                    release.bundle(self.artifacts, output)
                self.assertFalse((output / f'bin/{platform}/{member}').exists())
                self.assertFalse((output / 'checksums.json').exists())

    def test_existing_stage_is_never_overwritten(self):
        self.archives()
        self.output.mkdir()
        sentinel = self.output / 'keep.txt'
        sentinel.write_bytes(b'pre-existing content')
        with self.assertRaises(FileExistsError):
            release.bundle(self.artifacts, self.output)
        self.assertEqual(sentinel.read_bytes(), b'pre-existing content')
        self.assertEqual(list(self.output.iterdir()), [sentinel])

    def test_duplicate_binary_members_are_rejected(self):
        for platform in self.binaries:
            with self.subTest(platform=platform):
                self.archives()
                member = 'ttui.exe' if platform == 'win32-x64' else 'ttui'
                self.archive(platform, [(member, b'first', 'file'), (member, b'second', 'file')])
                output = self.root / f'duplicate-{platform}'
                with self.assertRaisesRegex(AssertionError, 'exactly one'):
                    release.bundle(self.artifacts, output)
                self.assertFalse((output / f'bin/{platform}/{member}').exists())

    def test_link_binary_members_are_rejected(self):
        for platform in self.binaries:
            with self.subTest(platform=platform):
                self.archives()
                member = 'ttui.exe' if platform == 'win32-x64' else 'ttui'
                self.archive(platform, [('alternative', b'wrong executable', 'file'),
                                        (member, b'alternative', 'link')])
                output = self.root / f'link-{platform}'
                with self.assertRaisesRegex(AssertionError, 'regular'):
                    release.bundle(self.artifacts, output)
                self.assertFalse((output / f'bin/{platform}/{member}').exists())

    def test_missing_binary_member_cannot_select_a_similarly_named_file(self):
        for platform in self.binaries:
            with self.subTest(platform=platform):
                self.archives()
                member = 'ttui.exe' if platform == 'win32-x64' else 'ttui'
                self.archive(platform, [(f'nested/{member}', b'wrong executable', 'file')])
                output = self.root / f'missing-{platform}'
                with self.assertRaisesRegex(AssertionError, 'exactly one'):
                    release.bundle(self.artifacts, output)
                self.assertFalse((output / f'bin/{platform}/{member}').exists())


if __name__ == '__main__':
    unittest.main()
