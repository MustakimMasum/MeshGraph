"""Exercise real flock/EXIT behavior with a controlled Docker command fixture."""

import gzip
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest


DOCKER_FIXTURE = r'''#!/usr/bin/env python3
import gzip, os, pathlib, sys, time
root = pathlib.Path(os.environ['BACKUP_FIXTURE'])
actor = os.environ['BACKUP_ACTOR']
args = sys.argv[1:]
def event(name):
    with (root / 'events').open('a') as f:
        f.write(actor + ':' + name + '\n')
def wait(name):
    (root / (actor + '.' + name)).touch()
    end = time.monotonic() + 15
    while not (root / ('release-' + name)).exists():
        if time.monotonic() > end:
            sys.exit(90)
        time.sleep(.02)
if args[0] == 'compose':
    command = args[args.index('-f') + 2]
    if command == 'ps':
        print('fixture-' + args[-1])
    elif command in ('stop', 'start'):
        event(command)
        if command == 'start' and os.environ.get('BLOCK_RESTART'):
            wait('restart')
elif args[0] == 'inspect':
    print(os.environ.get('BACKUP_VOLUME', 'shared-volume') if '.Mounts' in args[2] else 'true')
elif args[0] == 'run':
    event('copy')
    if os.environ.get('FAIL_COPY'):
        sys.exit(22)
    if os.environ.get('BLOCK_COPY'):
        wait('copy')
    sys.stdout.buffer.write(gzip.compress(b'consistent fixture snapshot'))
'''


@unittest.skipIf(os.name == 'nt', 'Run on Linux/WSL to exercise flock')
class BackupLockTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.script = Path(__file__).resolve().parents[1] / 'deploy/backup.sh'
        docker = self.root / 'docker'
        docker.write_text(DOCKER_FIXTURE, encoding='utf-8')
        docker.chmod(0o755)
        self.workdirs = [self.root / 'checkout-a', self.root / 'checkout-b']
        for workdir in self.workdirs:
            workdir.mkdir()

    def start(self, actor, checkout=0, **extra):
        env = dict(os.environ, PATH=f'{self.root}:{os.environ["PATH"]}',
                   BACKUP_FIXTURE=str(self.root), BACKUP_ACTOR=actor,
                   MESHGRAPH_BACKUP_LOCK_DIR=str(self.root / 'locks'), **extra)
        output = self.root / f'{actor}.tar.gz'
        compose = 'docker-compose.cloud.yml' if checkout == 0 else 'docker-compose.vps.yml'
        process = subprocess.Popen(['sh', str(self.script), compose, str(output)],
                                   cwd=self.workdirs[checkout], env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.addCleanup(self.stop_process, process)
        return process

    @staticmethod
    def stop_process(process):
        if process.poll() is None:
            process.kill()
        process.communicate(timeout=20)

    def wait_marker(self, name):
        end = time.monotonic() + 10
        while not (self.root / name).exists():
            if time.monotonic() > end:
                self.fail(f'Did not reach {name}')
            time.sleep(.02)

    def finish(self, process, expected=0):
        stdout, stderr = process.communicate(timeout=20)
        self.assertEqual(process.returncode, expected, (stdout + stderr).decode())

    def test_overlapping_checkouts_are_rejected_through_service_restart(self):
        first = self.start('first', BLOCK_COPY='1', BLOCK_RESTART='1')
        self.wait_marker('first.copy')
        self.finish(self.start('second', checkout=1), 75)
        self.assertFalse((self.root / 'second.tar.gz.partial').exists())
        (self.root / 'release-copy').touch()
        self.wait_marker('first.restart')
        self.finish(self.start('third', checkout=1), 75)
        (self.root / 'release-restart').touch()
        self.finish(first)
        self.finish(self.start('after', checkout=1))
        events = (self.root / 'events').read_text()
        self.assertNotIn('second:', events)
        self.assertNotIn('third:', events)
        self.assertEqual(gzip.decompress((self.root / 'after.tar.gz').read_bytes()), b'consistent fixture snapshot')

    def test_copy_failure_restarts_services_and_releases_lock(self):
        self.finish(self.start('failed', FAIL_COPY='1'), 22)
        self.assertIn('failed:start', (self.root / 'events').read_text())
        self.finish(self.start('retry'))

    def test_different_database_volumes_can_back_up_independently(self):
        first = self.start('first', BLOCK_COPY='1', BACKUP_VOLUME='volume-a')
        self.wait_marker('first.copy')
        self.finish(self.start('other', checkout=1, BACKUP_VOLUME='volume-b'))
        (self.root / 'release-copy').touch()
        self.finish(first)


if __name__ == '__main__':
    unittest.main()
