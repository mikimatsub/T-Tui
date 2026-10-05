#!/usr/bin/env python3
"""Real-terminal regression suite; isolated settings, cache, and fictional data.
Run: uv run --with pyte --with pillow python scripts/pty_e2e.py
"""
import argparse
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import termios
import tempfile
import time

import pyte

ROOT = Path(__file__).resolve().parents[1]

class TerminalApp:
    def __init__(self, binary, home, args, width=116, height=36, graphics=False):
        self.width, self.height = width, height
        self.screen = pyte.Screen(width, height)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder('utf-8')('replace')
        self.raw = bytearray()
        self.graphics = graphics
        self.answered_query = False
        self.graphics_pending = bytearray()
        self.sixels = []
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.environ.pop('NO_COLOR', None)
            os.environ.update(XDG_CONFIG_HOME=str(home / 'config'), XDG_CACHE_HOME=str(home / 'cache'), TERM='xterm-256color', COLORTERM='truecolor')
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', height, width, width*10 if graphics else 0, height*20 if graphics else 0))
            os.execv(str(binary), [str(binary), *args])
        self.closed = False
        self.resize(width, height)

    def resize(self, width, height):
        self.width, self.height = width, height
        self.screen.resize(height, width)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack('HHHH', height, width, width*10 if self.graphics else 0, height*20 if self.graphics else 0))
        os.kill(self.pid, signal.SIGWINCH)

    def pump(self, seconds=0.15):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            ready, _, _ = select.select([self.fd], [], [], min(.05, max(0, end-time.monotonic())))
            if ready:
                try: data = os.read(self.fd, 65536)
                except OSError: return
                if not data: return
                self.raw.extend(data)
                if self.graphics:
                    if not self.answered_query and b'\x1b[5n' in self.raw:
                        os.write(self.fd, b'\x1b[?62;4;22c\x1b[6;20;10t\x1b[0n')
                        self.answered_query = True
                    # Pyte models text, not Sixel. Preserve the image sequences
                    # separately and let it interpret only their surrounding text.
                    self.graphics_pending.extend(data)
                    while self.graphics_pending:
                        start = self.graphics_pending.find(b'\x1bP')
                        if start < 0:
                            # Keep a trailing escape until its next byte arrives.
                            keep = int(self.graphics_pending.endswith(b'\x1b'))
                            length = len(self.graphics_pending)-keep
                            self.stream.feed(self.decoder.decode(bytes(self.graphics_pending[:length])))
                            del self.graphics_pending[:length]
                            break
                        self.stream.feed(self.decoder.decode(bytes(self.graphics_pending[:start])))
                        del self.graphics_pending[:start]
                        sequence_end = self.graphics_pending.find(b'\x1b\\', 2)
                        if sequence_end < 0: break
                        sequence = bytes(self.graphics_pending[:sequence_end+2])
                        self.sixels.append(sequence)
                        del self.graphics_pending[:sequence_end+2]
                else:
                    self.stream.feed(self.decoder.decode(data))

    def send(self, data):
        os.write(self.fd, data.encode() if isinstance(data,str) else data)
        self.pump()

    def paste(self, text):
        self.send('\x1b[200~' + text + '\x1b[201~')

    def mouse(self, x, y, button=0):
        self.send(f'\x1b[<{button};{x+1};{y+1}M')
        if button < 64:
            self.send(f'\x1b[<{button};{x+1};{y+1}m')

    def click(self, label, occurrence=0):
        self.wait(label)
        for y,line in enumerate(self.screen.display):
            x=line.find(label)
            if x >= 0:
                if occurrence:
                    occurrence-=1
                    continue
                self.mouse(x+1,y)
                return
        raise AssertionError(f'No clickable label: {label}')

    @property
    def text(self):
        return '\n'.join(self.screen.display)

    def wait(self, text, timeout=8):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            self.pump()
            if text in self.text: return
        raise AssertionError(f'Missing {text!r}:\n{self.text}')

    def snapshot(self, destination):
        from PIL import Image, ImageDraw, ImageFont
        font_path = subprocess.check_output(['fc-match','-f','%{file}','monospace'], text=True)
        font = ImageFont.truetype(font_path, 16)
        cw, ch = 10, 22
        image = Image.new('RGB', (self.width*cw, self.height*ch), '#101218')
        draw = ImageDraw.Draw(image)
        colors = {'default':'e9e7e5','black':'101218','white':'e9e7e5','red':'ff7c89','green':'83ccad','blue':'89aaff','brown':'ddbb88','magenta':'ff7c89','cyan':'83ccad'}
        for y in range(self.height):
            for x in range(self.width):
                cell = self.screen.buffer[y][x]
                fg = colors.get(cell.fg,cell.fg); bg = '101218' if cell.bg == 'default' else colors.get(cell.bg,cell.bg)
                if cell.reverse: fg, bg = bg, fg
                try: draw.rectangle((x*cw,y*ch,(x+1)*cw,(y+1)*ch),fill='#'+bg)
                except ValueError: pass
                if cell.data == '▀':
                    draw.rectangle((x*cw,y*ch,(x+1)*cw-1,y*ch+ch//2-1), fill='#'+fg)
                else:
                    try: draw.text((x*cw,y*ch),cell.data,font=font,fill='#'+fg)
                    except ValueError: pass
        destination.parent.mkdir(parents=True,exist_ok=True)
        image.save(destination)
        destination.with_suffix('.txt').write_text(self.text)

    def quit(self):
        self.send('\x11')
        end=time.monotonic()+5
        while time.monotonic()<end:
            pid,status=os.waitpid(self.pid,os.WNOHANG)
            if pid:
                self.closed=True
                assert os.waitstatus_to_exitcode(status)==0
                assert b'panicked' not in self.raw
                assert b'\x1b[?1049l' in self.raw, 'alternate screen was not restored'
                os.close(self.fd)
                return
            self.pump()
        raise AssertionError('quit hung')

    def cleanup(self):
        if not self.closed:
            try: os.kill(self.pid,signal.SIGTERM); os.waitpid(self.pid,0)
            except ProcessLookupError: pass
            os.close(self.fd); self.closed=True


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/ttui')
    parser.add_argument('--artifacts',type=Path,default=Path('/tmp/ttui-qa'))
    args=parser.parse_args()
    count=0
    def passed(name):
        nonlocal count
        count+=1
        print(f'PASS {count:02d}  {name}',flush=True)
    with tempfile.TemporaryDirectory(prefix='ttui-e2e-') as temp:
        home=Path(temp)
        config=home/'config/ttui/demo.json'
        app=TerminalApp(args.binary.resolve(),home,['--mock'])
        try:
            app.wait('Ava');app.wait('I was literally');app.pump(2)
            app.snapshot(args.artifacts/'inbox.png');passed('inbox, message previews and asynchronous photos')
            app.send('\x1b[F');app.wait('Owen');passed('last conversation stays visible')
            app.send('/Ava\r');app.send('\r');app.wait("hey! how's your week going?")
            app.send('jklq?');app.paste(' 日本語\ncoffee?');app.wait('coffee?')
            assert 'jklq?' in app.text
            app.send('\x1b');app.wait('A closer look')
            saved=json.loads(config.read_text());assert any('jklq? 日本語\ncoffee?'==v for v in saved['drafts'].values());passed('normal typing, Unicode, bracketed paste and saved draft')
            app.send('\r');app.wait('coffee?');app.send('\x15');app.send('jklq? hello from terminal');app.send('\r');app.wait('jklq? hello from terminal')
            app.wait("haha that's actually really good",timeout=12)
            app.snapshot(args.artifacts/'chat.png');passed('send acknowledgement and incoming reply')
            app.send('\x10');app.wait('About');app.wait('THE LITTLE THINGS');app.pump(.4)
            app.send('\x1b[C');app.wait('Photos  2 /')
            app.snapshot(args.artifacts/'profile.png');passed('profile details and photo carousel')
            app.send('\x1b');app.send('\x1b');app.send('\x1b');app.send('d');app.wait('Felix');app.pump(1)
            app.snapshot(args.artifacts/'discover.png')
            app.send('n');app.wait('Sara');app.send('yy');app.wait("It's a match!")
            assert 'Jonas' in app.text
            passed('discovery pass, like, new match and duplicate-submit guard')
            app.send('m');app.wait('Sara');passed('new discovery match reaches inbox')
            app.send('s');app.wait('Make it yours');app.send('\r');app.wait('Themes · live preview');app.send('\x1b[B\r');app.pump()
            assert json.loads(config.read_text())['theme']=='light'
            assert config.stat().st_mode & 0o777 == 0o600
            app.snapshot(args.artifacts/'settings-light.png');passed('theme persistence and owner-only configuration')
            app.send('jj\r');assert json.loads(config.read_text())['image_enabled'] is False
            app.send('\r');assert json.loads(config.read_text())['image_enabled'] is True
            passed('photo toggle can be disabled and re-enabled')
            app.resize(50,16);app.send('kkkk');app.pump();assert 'Sign out' in app.text or 'External photo' in app.text or 'Theme' in app.text
            app.resize(20,5);app.wait('T-TUI');app.resize(116,36);app.wait('Make it yours');passed('live terminal resizing')
            app.send('\x1b');app.send('?');app.wait('At your fingertips');app.send('\x1b');passed('help overlay opens and dismisses')
            app.send('s');app.send('j'*11);app.pump()
            app.send('\x1b');app.send('/Ava\r\r');app.wait('Message')
            app.paste('keep this for tomorrow?');app.quit();passed('clean exit restores terminal')
        finally:app.cleanup()
        app=TerminalApp(args.binary.resolve(),home,['--mock'])
        try:
            app.wait('Ava');app.send('/Ava\r\r');app.wait('keep this for tomorrow?');passed('draft survives application restart')
            app.quit()
        finally:app.cleanup()
        app=TerminalApp(args.binary.resolve(),home,['--live'])
        try:
            app.wait('Welcome to T-TUI');assert 'DEMO' not in app.text
            app.paste("curl 'https://api.gotinder.com/v2/matches' -H 'x-auth-token: hidden-token-example' -H 'persistent-device-id: local-device'")
            app.wait('local-device');assert 'hidden-token-example' not in app.text;assert 'curl' not in app.text
            app.snapshot(args.artifacts/'login.png');passed('live/demo isolation and masked one-paste session import')
            app.quit();assert not (home/'config/ttui/config.json').exists()
        finally:app.cleanup()
        app=TerminalApp(args.binary.resolve(),home,['--mock'])
        try:
            app.wait('Ava')
            assert b'\x1b[?1006h' in app.raw, 'SGR mouse capture not enabled'
            app.click('[Themes]');app.wait('Themes · live preview')
            app.click('Dracula');app.click('[Apply]')
            assert json.loads(config.read_text())['theme']=='dracula'
            app.click('[Themes]');app.click('Catppuccin Mocha')
            app.snapshot(args.artifacts/'themes-catppuccin.png')
            app.click('[Cancel]');assert json.loads(config.read_text())['theme']=='dracula'
            passed('mouse theme preview, apply and cancel')
            app.click('[Discover]');app.wait('Felix')
            app.click('[▶]');app.wait('Photos  2 /')
            app.click('[u Super Like]');app.wait('Requires account credits')
            app.click('[Cancel]');app.wait('Felix')
            app.click('[u Super Like]');app.click('[Confirm]');app.wait('Super Like confirmed')
            app.click('[b Boost]');app.click('[Confirm]');app.wait('Boost activated')
            passed('mouse carousel and confirmed premium actions in demo')
            app.click('[Account]');app.wait('My account');app.wait('Click a field')
            # Click the bio text field, replace via paste, and save explicitly.
            for y,line in enumerate(app.screen.display):
                if 'Bio ·' in line:
                    app.mouse(line.index('Bio')+1,y+1);break
            app.send('\x15');app.paste('Mouse edited bio 🦀')
            app.wait('Unsaved changes');app.snapshot(args.artifacts/'account-dracula.png')
            app.click('[Save]');app.wait('Account changes saved')
            app.click('[Reload]');app.wait('Mouse edited bio')
            app.resize(50,16);app.pump();app.snapshot(args.artifacts/'account-small.png')
            app.resize(116,36);app.pump()
            passed('mouse account editing, explicit save, reload and resize')
            app.click('[Inbox');app.click('[Clear]');app.click('[Find]');app.send('Ava\r')
            app.click('Ava',occurrence=1);app.wait('Ctrl-P profile')
            app.click('[Unmatch]');app.wait('undone.')
            app.click('[Cancel]');app.wait('Ctrl-P profile')
            app.click('[Unmatch]');app.wait('undone.');app.click('[Unmatch]')
            app.wait('conversation has been removed')
            passed('mouse unmatch cancellation and confirmed removal')
            app.click('[Help]');app.wait('At your fingertips')
            app.mouse(10,10,65);app.click('[Close]')
            app.quit()
            assert b'\x1b[?1006l' in app.raw, 'mouse capture was not restored'
            passed('mouse help scrolling and terminal restoration')
        finally:app.cleanup()
        env = dict(os.environ, XDG_CONFIG_HOME=str(home/'config'), XDG_CACHE_HOME=str(home/'cache'))
        curl = "curl 'https://api.gotinder.com/v2/matches' -H 'x-auth-token: mock-import-token' -H 'persistent-device-id: mock-device'"
        imported = subprocess.run([str(args.binary.resolve()), '--mock', '--import-session'],input=curl,text=True,env=env,capture_output=True,timeout=10)
        assert imported.returncode == 0, imported.stderr
        assert 'mock-import-token' not in imported.stdout
        assert json.loads(config.read_text())['auth_token'] == 'mock-import-token'
        passed('stdin session import validates and stores only parsed credentials')
        checked = subprocess.run([str(args.binary.resolve()), '--mock', '--check'],text=True,env=env,capture_output=True,timeout=15)
        assert checked.returncode == 0, checked.stderr
        assert 'Read-only checks complete' in checked.stdout
        passed('read-only diagnostic covers all fetch paths')
        app=TerminalApp(args.binary.resolve(),home,['--mock'],graphics=True)
        try:
            app.wait('Ava');app.pump(2)
            assert app.answered_query and len(app.sixels)>=2, 'native graphics were not detected/rendered'
            sizes=[tuple(map(int,re.search(rb'"\d+;\d+;(\d+);(\d+)',s).groups())) for s in app.sixels]
            assert any(w>=200 and h>=200 for w,h in sizes), sizes
            count_before=len(app.sixels);app.pump(.5)
            assert len(app.sixels)==count_before, 'unchanged photos are retransmitted on every tick'
            app.send('s');app.wait('Sixel');app.send('\x1b')
            passed('Sixel detection, real pixel resolution and stable-frame caching')
            app.send('?');app.wait('At your fingertips');app.pump(.3)
            count_before=len(app.sixels);app.pump(.4)
            assert len(app.sixels)==count_before, 'photos render over help'
            app.send('\x1b');app.pump(.3);assert len(app.sixels)>count_before
            app.click('[Themes]');app.wait('Themes · live preview');app.pump(.3)
            count_before=len(app.sixels);app.pump(.4)
            assert len(app.sixels)==count_before, 'photos render over theme picker'
            app.click('[Cancel]');app.pump(.3)
            assert len(app.sixels)>count_before
            passed('native images stay beneath theme picker')
            app.send('d');app.wait('y like');app.pump(.7)
            count_before=len(app.sixels);app.send('\x1b[C');app.pump(.7)
            assert len(app.sixels)>count_before, 'carousel did not replace native photo'
            passed('native image cleanup across help, navigation and carousel')
            app.resize(80,24);app.pump(.7)
            assert len(app.sixels)>count_before
            app.send('m');app.send('s');app.wait('Sixel');app.send('jj\r')
            app.send('\x1b');app.pump(.2);count_before=len(app.sixels);app.pump(.4)
            assert len(app.sixels)==count_before, 'photos still render while disabled'
            app.send('s');app.send('\r');app.send('\x1b');app.pump(.7)
            assert len(app.sixels)>count_before, 'photos did not return after re-enabling'
            app.quit();passed('native image resize, pause/resume and clean exit')
        finally:app.cleanup()
    print(f'All {count} terminal checks passed. Screenshots: {args.artifacts}',flush=True)

if __name__=='__main__':main()
