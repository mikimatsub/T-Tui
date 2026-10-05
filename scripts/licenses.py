"""Collect upstream license/notice texts for the locked Linux and Windows graphs."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from release import ROOT, TARGETS


def normalized(text):
    return '\n'.join(line.rstrip() for line in text.splitlines()).strip() + '\n'


def collect():
    packages = {}
    for target in TARGETS.values():
        metadata = json.loads(subprocess.check_output(
            ['cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', target],
            cwd=ROOT, text=True))
        nodes = {node['id']: node for node in metadata['resolve']['nodes']}
        by_id = {package['id']: package for package in metadata['packages']}
        seen, pending = set(), [metadata['resolve']['root']]
        while pending:
            key = pending.pop()
            if key in seen:
                continue
            seen.add(key)
            pending.extend(dep['pkg'] for dep in nodes[key]['deps']
                           if any(kind['kind'] != 'dev' for kind in dep['dep_kinds']))
        for key in seen - {metadata['resolve']['root']}:
            package = by_id[key]
            packages[(package['name'], package['version'])] = package
    sources = json.loads((ROOT / 'docs/license-sources/sources.json').read_text(encoding='utf-8-sig'))
    texts, entries = {}, []
    for (name, version), package in sorted(packages.items()):
        root = Path(package['manifest_path']).parent
        files = sorted(path for path in root.rglob('*') if path.is_file() and not path.is_symlink()
                       and (re.match(r'^(licen[sc]e|notice|copying|copyright)([._-]|$)', path.name, re.I)
                            or any(part.lower() in ('licenses', 'license') for part in path.relative_to(root).parts[:-1])))
        key = f'{name}-{version}'
        origin = f'https://crates.io/crates/{name}/{version}'
        if not files:
            assert key in sources, f'Missing upstream license texts: {key}'
            root = ROOT / 'docs/license-sources' / key
            files = sorted(path for path in root.iterdir() if path.is_file())
            origin += '\nSupplemental license sources: ' + ', '.join(sources[key])
        assert files and package['license'], f'Missing license information: {key}'
        refs = []
        for path in files:
            content = normalized(path.read_text(encoding='utf-8-sig')).strip()
            fingerprint = hashlib.sha256(content.encode()).hexdigest()
            texts.setdefault(fingerprint, content)
            refs.append(f'  {path.relative_to(root).as_posix()}: {fingerprint}')
        entries.append(f'{name} {version}\nDeclared license: {package["license"]}\nSource: {origin}\n' + '\n'.join(refs))
    intro = ('Third-party notices for T-TUI\n\nGenerated from locked Linux and Windows dependency graphs, including build dependencies.\n'
             'Trailing whitespace is normalized; identical texts are stored once and referenced by SHA-256. License alternatives remain alternatives.\n'
             'The modified Crossterm source and patch description are in vendor/crossterm.\n'
             'Rust standard-library notices are in RUST-STDLIB-LICENSE.html, copied from the pinned toolchain.\n\n')
    return intro + '\n\n'.join(entries) + '\n\nLICENSE TEXTS\n\n' + '\n\n'.join(
        f'=== {key} ===\n{content}' for key, content in sorted(texts.items())) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    result = collect()
    notice = ROOT / 'THIRD_PARTY_NOTICES.txt'
    sysroot = Path(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip())
    stdlib = normalized((sysroot / 'share/doc/rust/COPYRIGHT-library.html').read_text(encoding='utf-8'))
    if args.check:
        assert notice.read_text(encoding='utf-8') == result, 'Regenerate dependency notices: python scripts/licenses.py'
        assert (ROOT / 'RUST-STDLIB-LICENSE.html').read_text(encoding='utf-8') == stdlib, 'Regenerate Rust library notices'
    else:
        notice.write_text(result, encoding='utf-8', newline='\n')
        (ROOT / 'RUST-STDLIB-LICENSE.html').write_text(stdlib, encoding='utf-8', newline='\n')
    print('Locked dependency and Rust standard-library notices verified')


if __name__ == '__main__':
    main()
