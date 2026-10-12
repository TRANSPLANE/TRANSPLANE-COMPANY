#!/usr/bin/env python3
"""Trusted local backup destination; one JSON request on stdin, one reply on stdout."""
import hashlib
import json
import os
import shutil
import sys
import tempfile
from pathlib import Path


def digest(path):
    hasher = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            hasher.update(block)
    return hasher.hexdigest()


def deliver(payload):
    source = Path(payload['snapshot']).resolve(strict=True)
    if not source.is_file():
        raise ValueError('snapshot must be a file')
    destination = Path(payload['config']['destination']).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    target = destination / source.name
    expected = digest(source)
    if target == source:
        return {'status': 'delivered', 'path': str(target), 'sha256': expected}
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination, prefix='.company-backup-', delete=False) as output:
            temporary = Path(output.name)
            with source.open('rb') as original:
                shutil.copyfileobj(original, output, length=1024 * 1024)
            output.flush()
            os.fsync(output.fileno())
        if digest(temporary) != expected:
            raise ValueError('copied snapshot checksum mismatch')
        os.replace(temporary, target)
        temporary = None
        return {'status': 'delivered', 'path': str(target), 'sha256': expected}
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main():
    try:
        payload = json.loads(sys.stdin.readline())
        result = deliver(payload)
        print(json.dumps(result, ensure_ascii=False))
        return 0
    except Exception as exc:
        print(json.dumps({'status': 'failed', 'error': str(exc)}, ensure_ascii=False))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
