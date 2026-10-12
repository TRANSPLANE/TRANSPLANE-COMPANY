#!/usr/bin/env python3
"""Check a packaged CLI, launcher and embedded server on the selected host or QEMU runner."""
from __future__ import annotations

import argparse
import hashlib
import json
import shlex
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary-dir', type=Path, required=True)
    parser.add_argument('--runner', default='', help='Optional QEMU command, for example qemu-arm -L /usr/arm-linux-gnueabihf')
    args = parser.parse_args()
    directory = args.binary_dir.resolve()
    extension = '.exe' if (directory / 'company.exe').exists() else ''
    runner = shlex.split(args.runner)
    manifest = directory / 'release-manifest.json'
    if manifest.exists():
        for relative, expected in json.loads(manifest.read_text(encoding='utf-8'))['files'].items():
            file = directory / relative
            if not file.resolve().is_relative_to(directory) or file.is_symlink():
                raise RuntimeError('The release manifest contains an invalid file path.')
            if hashlib.sha256(file.read_bytes()).hexdigest() != expected:
                raise RuntimeError('A packaged file failed its SHA-256 verification.')

    def command(name: str, *arguments: str) -> list[str]:
        return [*runner, str(directory / (name + extension)), *arguments]

    def run(name: str, *arguments: str) -> bytes:
        result = subprocess.run(command(name, *arguments), stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
        if result.returncode:
            raise RuntimeError(f'{name} failed its release smoke check (exit {result.returncode}).')
        return result.stdout

    for binary in ('company', 'company-server', 'company-launch'):
        run(binary, '--help')
    with tempfile.TemporaryDirectory(prefix='transplane-release-smoke-') as temporary:
        data_dir = str(Path(temporary) / 'data')
        initial = json.loads(run('company', '--data-dir', data_dir, 'init'))
        token = initial.get('owner_token')
        if not isinstance(token, str) or not token:
            raise RuntimeError('First initialization did not create an owner credential.')
        repeated = json.loads(run('company', '--data-dir', data_dir, 'init'))
        if repeated.get('owner_token') or repeated.get('owner_token_created') is not False:
            raise RuntimeError('Repeated initialization unexpectedly changed the owner credential.')
        if json.loads(run('company', '--data-dir', data_dir, 'companies')).get('companies') != []:
            raise RuntimeError('The clean deployment contains unexpected company data.')
        for program in ('company-server', 'company-launch'):
            session_data = data_dir if program == 'company-server' else str(Path(temporary) / 'launcher-data')
            with socket.socket() as available:
                available.bind(('127.0.0.1', 0))
                port = available.getsockname()[1]
            base = f'http://127.0.0.1:{port}'
            log_path = Path(temporary) / (program + '.log')
            with log_path.open('wb') as log:
                arguments = ['--data-dir', session_data, '--bind', f'127.0.0.1:{port}']
                if program == 'company-launch':
                    arguments.append('--no-browser')
                server = subprocess.Popen(command(program, *arguments), stdout=log, stderr=log)
                try:
                    deadline = time.monotonic() + 75
                    while True:
                        if server.poll() is not None:
                            raise RuntimeError(f'{program} exited before becoming ready.')
                        try:
                            with urllib.request.urlopen(base + '/api/v1/health', timeout=2) as response:
                                health = json.load(response)
                            if health.get('status') != 'ok':
                                raise RuntimeError('The server health response is invalid.')
                            if manifest.exists() and health.get('version') != json.loads(manifest.read_text(encoding='utf-8'))['version']:
                                raise RuntimeError('The packaged server version does not match the release manifest.')
                            break
                        except (urllib.error.URLError, TimeoutError):
                            if time.monotonic() >= deadline:
                                raise RuntimeError(f'{program} did not become ready in time.')
                            time.sleep(0.25)
                    session_token = token
                    if program == 'company-launch':
                        ready = None
                        for line in log_path.read_text(encoding='utf-8', errors='replace').splitlines():
                            try:
                                value = json.loads(line)
                            except json.JSONDecodeError:
                                continue
                            if value.get('status') == 'running':
                                ready = value
                                break
                        if not ready or not ready.get('owner_token') or ready.get('url') != base:
                            raise RuntimeError('First launcher startup did not return owner access and the correct service address.')
                        session_token = ready['owner_token']
                        reopened = json.loads(run('company-launch', *arguments))
                        if reopened.get('status') != 'already_running' or reopened.get('url') != base:
                            raise RuntimeError('Repeated launcher startup did not reconnect to the original service.')
                        if json.loads(run('company', '--data-dir', session_data, 'init')).get('owner_token_created') is not False:
                            raise RuntimeError('Launcher initialization did not preserve owner access.')
                    with urllib.request.urlopen(base + '/', timeout=10) as response:
                        if '<html' not in response.read().decode('utf-8').lower():
                            raise RuntimeError(f'{program} did not serve its embedded interface.')
                    with urllib.request.urlopen(base + '/api/v1/agent-entry?lang=en', timeout=10) as response:
                        entry = json.load(response)
                        if not entry.get('discovery') or not entry.get('player_output'):
                            raise RuntimeError('Agent onboarding is missing from the packaged service.')
                    request = urllib.request.Request(base + '/api/v1/companies', headers={'Authorization': 'Bearer ' + session_token})
                    with urllib.request.urlopen(request, timeout=10) as response:
                        if json.load(response).get('companies') != []:
                            raise RuntimeError('The service contains unexpected company data.')
                finally:
                    server.terminate()
                    try:
                        server.wait(timeout=15)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait(timeout=10)
    checks = 'program entry points, repeat-safe initialization, launcher startup/reconnection, authorized access, Agent entry and embedded interface'
    if manifest.exists():
        checks = 'package hashes and version, ' + checks
    print('PASS: ' + checks + '.')


if __name__ == '__main__':
    main()
