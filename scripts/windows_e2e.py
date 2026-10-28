"""Native Windows ConPTY smoke with isolated, fictional account data."""
import argparse
import json
import os
from pathlib import Path
import select
import tempfile
import time
import pyte
from winpty import PtyProcess

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    args = parser.parse_args()
    evidence = Path('target/terminal-evidence')
    evidence.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='ttui-conpty-') as temporary:
        root = Path(temporary)
        (root / 'config').mkdir()
        config = root / 'config/demo.json'
        config.write_text(json.dumps({'mock': True, 'image_enabled': False}))
        env = dict(os.environ, TTUI_DATA_DIR=str(root), TERM='xterm-256color')
        proc = PtyProcess.spawn([str(args.binary.resolve()), '--mock'], env=env, dimensions=(36, 116))
        screen = pyte.Screen(116, 36)
        stream = pyte.Stream(screen)
        raw = []
        def pump(seconds=0.2):
            end = time.monotonic() + seconds
            while time.monotonic() < end:
                ready, _, _ = select.select([proc.fileobj], [], [], min(.05, max(0, end-time.monotonic())))
                if ready:
                    try: data = proc.read(65536)
                    except EOFError: return
                    raw.append(data)
                    stream.feed(data)
        def wait(text):
            end = time.monotonic() + 12
            while time.monotonic() < end:
                pump()
                if text in '\n'.join(screen.display): return
            raise AssertionError(f'Missing {text!r}\n' + '\n'.join(screen.display))
        def wait_gone(text):
            # Avoid matching a stale frame: wait until the previous screen is gone.
            end = time.monotonic() + 12
            while time.monotonic() < end:
                pump(.1)
                if text not in '\n'.join(screen.display): return
            raise AssertionError(f'Still showing {text!r}\n' + '\n'.join(screen.display))
        def drafts():
            try: return json.loads(config.read_text(encoding='utf-8')).get('drafts', {})
            except (OSError, ValueError): return None  # file mid-write; poll again
        def wait_drafts(check, message, timeout=10):
            end = time.monotonic() + timeout
            while time.monotonic() < end:
                pump(.1)
                current = drafts()
                if current is not None and check(current): return
            raise AssertionError(f'{message}: {drafts()!r}\n' + '\n'.join(screen.display))
        def send(text):
            proc.write(text)
            pump()
        try:
            wait('Ava')
            wait('Conversations')
            send('\r')
            wait('Ctrl-P profile')
            send('\x1b[200~Windows draft 日本語 🦀\nsecond line\x1b[201~')
            wait('Windows draft')
            send('\x1b')
            wait_gone('Ctrl-P profile')
            wait('Conversations')
            draft = 'Windows draft 日本語 🦀\nsecond line'
            wait_drafts(lambda d: draft in d.values(), 'Draft was not persisted')
            send('\r')
            wait('Ctrl-P profile')
            # The composer is ready once the restored draft is shown in the Message box.
            end = time.monotonic() + 12
            while 'Windows draft' not in '\n'.join(screen.display[-8:]):
                if time.monotonic() > end: raise AssertionError('Draft not restored in composer\n' + '\n'.join(screen.display))
                pump(.1)
            send('\x13')
            wait_drafts(lambda d: not any(d.values()), 'Ctrl-S did not send the demo draft')
            wait('Write something')
            send('\x1b')
            wait_gone('Ctrl-P profile')
            send('s')
            wait('Sync interval')
            send('\x1b')
            send('d')
            wait('y like')
            # Demo-only actions exercise keyboard dispatch and duplicate-key protection.
            send('y')
            pump(.5)
            send('m')
            wait('Conversations')
            send('?')
            wait('At your fingertips')
            send('\x1b')
            proc.setwinsize(24, 80)
            screen.resize(24, 80)
            pump(.5)
            wait('Conversations')
            (evidence / 'windows-demo.txt').write_text('\n'.join(screen.display), encoding='utf-8')
            send('\x11')
            deadline = time.monotonic() + 5
            while proc.isalive() and time.monotonic() < deadline: pump(.1)
            assert not proc.isalive(), 'Ctrl-Q did not terminate the app'
            assert proc.exitstatus == 0, f'Exit code {proc.exitstatus}'
            assert '\x1b[?1049l' in ''.join(raw), 'Alternate screen was not restored'
            assert not (root / 'config/config.json').exists(), 'Demo wrote live account config'
            print('Windows ConPTY: startup, navigation, Unicode draft persistence, demo discovery, help, resize, clean exit and isolation passed.')
        finally:
            (evidence / 'windows-last-frame.txt').write_text('\n'.join(screen.display), encoding='utf-8')
            if proc.isalive(): proc.terminate(force=True)
            proc.close()

if __name__ == '__main__': main()
