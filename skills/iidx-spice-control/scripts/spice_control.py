"""Small official-spiceapi wrapper; credentials come from the environment."""
import argparse
from contextlib import contextmanager
import importlib
import os
from pathlib import Path
import socket
import sys
import time


class Client:
    def __init__(self, api, connection):
        self.api = api
        self.connection = connection
        self._held = set()

    @classmethod
    def from_env(cls, host=None, port=None, api_python=None):
        host = host or os.environ.get('IIDX_SPICE_HOST')
        password = os.environ.get('IIDX_SPICE_PASSWORD')
        if not host or password is None:
            raise ValueError('Set IIDX_SPICE_HOST and IIDX_SPICE_PASSWORD')
        location = api_python or os.environ.get('IIDX_SPICE_API_PYTHON')
        if location:
            sys.path.insert(0, str(Path(location).resolve()))
        api = importlib.import_module('spiceapi')
        port = port or int(os.environ.get('IIDX_SPICE_PORT', '55573'))
        return cls(api, api.Connection(host, port, password))

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.connection.close()

    @contextmanager
    def held(self, *names):
        names = list(dict.fromkeys(names))
        available = {entry[0] for entry in self.api.buttons_read(self.connection)}
        missing = set(names) - available
        if missing:
            raise ValueError('Unknown buttons: ' + ', '.join(sorted(missing)))
        # Nested chords must not release an outer Start hold.
        added = [name for name in names if name not in self._held]
        self._held.update(added)
        try:
            if added:
                self.api.buttons_write(self.connection, [[name, 1] for name in added])
            yield
        finally:
            try:
                if added:
                    self.api.buttons_write_reset(self.connection, [[name] for name in added])
            finally:
                self._held.difference_update(added)

    def press(self, *names, duration=0.55):
        if not 0 < duration <= 10:
            raise ValueError('Press duration must be within (0, 10] seconds')
        with self.held(*names):
            time.sleep(duration)

    def touch(self, x, y, contact=99, duration=0.2):
        if not 0 < duration <= 10 or contact < 0:
            raise ValueError('Invalid contact or duration')
        try:
            self.api.touch_write(self.connection, [[contact, x, y]])
            time.sleep(duration)
        finally:
            self.api.touch_write_reset(self.connection, [contact])

    def capture(self, output, screen=0):
        frame = self.api.capture_get_jpg(self.connection, screen, 90, 1)
        if not frame:
            raise RuntimeError('No captured frame for screen ' + str(screen))
        output = Path(output)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(frame['data'])
        print(f"screen {screen}: {frame['width']}x{frame['height']} -> {output}")

    def process(self, restart=False):
        host, port = self.connection.host, self.connection.port
        action = self.api.control_restart if restart else self.api.control_exit
        try:
            action(self.connection)
        except (ConnectionError, TimeoutError, OSError):
            # Losing the old process is expected; it is not proof of completion.
            pass
        if restart:
            print('Restart sent once; verify a new connection and startup log.')
            return
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            try:
                with socket.create_connection((host, port), timeout=1):
                    pass
            except OSError:
                print('Game exit requested; Spice API is offline.')
                return
            time.sleep(0.5)
        raise RuntimeError('Spice API still listening; inspect before retrying exit')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--host')
    parser.add_argument('--port', type=int)
    parser.add_argument('--api-python', type=Path)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('buttons')
    capture = commands.add_parser('capture')
    capture.add_argument('output', type=Path)
    capture.add_argument('--screen', type=int, default=0)
    press = commands.add_parser('press')
    press.add_argument('names', nargs='+')
    press.add_argument('--duration', type=float, default=0.55)
    touch = commands.add_parser('touch')
    touch.add_argument('x', type=int)
    touch.add_argument('y', type=int)
    touch.add_argument('--contact', type=int, default=99)
    coin = commands.add_parser('coin')
    coin.add_argument('count', type=int, choices=range(1, 10))
    card = commands.add_parser('card')
    card.add_argument('card_id')
    card.add_argument('--unit', type=int, choices=(0, 1), default=0)
    commands.add_parser('exit')
    commands.add_parser('restart')
    args = parser.parse_args()
    if args.command == 'card' and (
        len(args.card_id) != 16 or any(c not in '0123456789abcdefABCDEF' for c in args.card_id)
    ):
        parser.error('Card ID must contain exactly 16 hexadecimal characters')
    with Client.from_env(args.host, args.port, args.api_python) as game:
        if args.command == 'buttons':
            for entry in game.api.buttons_read(game.connection):
                print(entry[0])
        elif args.command == 'capture':
            game.capture(args.output, args.screen)
        elif args.command == 'press':
            game.press(*args.names, duration=args.duration)
        elif args.command == 'touch':
            game.touch(args.x, args.y, args.contact)
        elif args.command == 'coin':
            game.api.coin_insert(game.connection, args.count)
        elif args.command == 'card':
            game.api.card_insert(game.connection, args.unit, args.card_id)
        else:
            game.process(restart=args.command == 'restart')


if __name__ == '__main__':
    main()
