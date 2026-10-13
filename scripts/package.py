#!/usr/bin/env python3
"""Package release programs and public user assets, excluding development and company data."""
from __future__ import annotations
import argparse
import hashlib
import json
import re
import shutil
import struct
import tarfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PLATFORMS = {
    'windows-x86_64': 'x86_64-pc-windows-msvc',
    'windows-aarch64': 'aarch64-pc-windows-msvc',
    'macos-x86_64': 'x86_64-apple-darwin',
    'macos-aarch64': 'aarch64-apple-darwin',
    'linux-x86_64': 'x86_64-unknown-linux-gnu',
    'linux-aarch64': 'aarch64-unknown-linux-gnu',
    'linux-x86_64-musl': 'x86_64-unknown-linux-musl',
    'linux-aarch64-musl': 'aarch64-unknown-linux-musl',
    'linux-armv7': 'armv7-unknown-linux-gnueabihf',
    'linux-i686': 'i686-unknown-linux-gnu',
    'linux-riscv64': 'riscv64gc-unknown-linux-gnu',
}
USER_DOCUMENTS = (
    'getting-started.md', 'getting-started.zh-CN.md',
    'agent-interface.en.md', 'agent-interface.md',
    'hosted-skills.en.md', 'hosted-skills.md',
    'plugins.en.md', 'plugins.md', 'operations.md',
    'studio.md', 'studio.zh-CN.md',
)
USER_SCRIPTS = (
    'agent-host.py',
    'capture-company.mjs', 'load-example.ps1',
    'renderer/capture-company.mjs', 'renderer/package.json', 'renderer/package-lock.json',
)
ROOT_DOCUMENTS = (
    'README.md', 'README.en.md', 'README.zh-CN.md',
    'LICENSE', 'NOTICE', 'THIRD_PARTY_NOTICES.md',
)
SKILL_ASSETS = (
    'transplane-company/SKILL.md', 'transplane-company/references/entry.en.md',
    'transplane-company/agents/openai.yaml',
    *(f'company-operating/{topic}/{name}'
      for topic in ('cashflow-review', 'customer-interviews', 'simulation-gm', 'weekly-retrospective')
      for name in ('SKILL.md', 'SKILL.en.md')),
)
EXAMPLE_ASSETS = (
    'demo-company.json', 'demo-operations.json',
    'simulation-company.json', 'simulation-company.en.json',
    'simulation-operations.json', 'simulation-operations.en.json',
    'plugins/backup-config.json', 'plugins/backup_destination.py',
    'plugins/business-summary.json', 'plugins/business_summary.py',
    'plugins/local-backup.json', 'plugins/operating-skills.json', 'plugins/process-backup.json',
)
LICENSE_ASSETS = (
    'fusion-pixel-OFL-1.1.txt', 'markdown-runtime-notices.txt',
    'pixelarticons-MIT.txt', 'playwright-Apache-2.0.txt',
)
MEDIA_ASSETS = (
    'hero.svg', 'hero-dark.svg', 'demo-en.png', 'demo-zh-CN.png',
    *(f'company-{kind}{theme}.{extension}'
      for kind, extensions in (('icon', ('svg', 'png', 'ico')), ('mark', ('svg', 'png')))
      for theme in ('', '-light', '-dark') for extension in extensions),
)


def regular_file(source: Path) -> None:
    if not source.is_file() or source.is_symlink():
        raise RuntimeError(f'Expected a regular release source file: {source.relative_to(ROOT)}')
    for ancestor in source.parents:
        if ancestor == ROOT:
            break
        if ancestor.is_symlink():
            raise RuntimeError('Packaging does not follow source symlinks.')
    if not source.resolve().is_relative_to(ROOT):
        raise RuntimeError('Release assets must remain inside the source checkout.')


def public_assets() -> list[Path]:
    assets = [ROOT / name for name in ROOT_DOCUMENTS]
    assets.extend(ROOT / 'docs' / name for name in USER_DOCUMENTS)
    assets.extend(ROOT / 'scripts' / name for name in USER_SCRIPTS)
    assets.extend(ROOT / 'skills' / name for name in SKILL_ASSETS)
    assets.extend(ROOT / 'examples' / name for name in EXAMPLE_ASSETS)
    assets.extend(ROOT / 'docs/licenses' / name for name in LICENSE_ASSETS)
    assets.extend(ROOT / 'docs/media' / name for name in MEDIA_ASSETS)
    for source in assets:
        regular_file(source)
    return sorted(set(assets))


def validate_binary(binary: Path, platform: str) -> None:
    content = binary.read_bytes()
    architecture = platform.split('-')[1]
    if platform.startswith('windows'):
        if content[:2] != b'MZ' or len(content) < 64:
            raise RuntimeError(f'{binary.name} is not a Windows executable.')
        header = struct.unpack_from('<I', content, 0x3C)[0]
        if content[header:header + 4] != b'PE\0\0':
            raise RuntimeError(f'{binary.name} has an invalid executable header.')
        actual = struct.unpack_from('<H', content, header + 4)[0]
        expected = {'x86_64': 0x8664, 'aarch64': 0xAA64}[architecture]
    elif platform.startswith('macos'):
        if content[:4] != b'\xcf\xfa\xed\xfe':
            raise RuntimeError(f'{binary.name} is not a supported Mach-O executable.')
        actual = struct.unpack_from('<I', content, 4)[0]
        expected = {'x86_64': 0x01000007, 'aarch64': 0x0100000C}[architecture]
    else:
        if content[:4] != b'\x7fELF' or content[5] != 1:
            raise RuntimeError(f'{binary.name} is not a supported little-endian ELF executable.')
        actual = struct.unpack_from('<H', content, 18)[0]
        expected = {'x86_64': 62, 'aarch64': 183, 'armv7': 40, 'i686': 3, 'riscv64': 243}[architecture]
    if actual != expected:
        raise RuntimeError(f'{binary.name} does not match the selected release architecture.')
    personal_path = re.compile(rb'(?:[A-Za-z]:[\\/]+Users[\\/]+[^\\/\x00\s]+|/(?:home|Users)/[^/\x00\s]+)')
    if personal_path.search(content) or personal_path.search(content.replace(b'\0', b'')):
        raise RuntimeError(f'{binary.name} contains a personal build path; rebuild with --remap-path-prefix.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=tuple(PLATFORMS), required=True)
    parser.add_argument('--binary-dir', type=Path, required=True)
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--version', required=True, help='Release version, for example 0.1.1')
    args = parser.parse_args()
    destination = args.destination.resolve()
    if destination == ROOT or (destination.is_relative_to(ROOT) and not destination.is_relative_to(ROOT / 'target')) or destination.exists():
        parser.error('Use a fresh release directory under target/ or outside the source checkout.')
    extension = '.exe' if args.platform.startswith('windows') else ''
    binaries = [args.binary_dir / (name + extension) for name in ('company', 'company-server', 'company-launch')]
    if not all(binary.is_file() and not binary.is_symlink() for binary in binaries):
        parser.error('All three regular release binaries must exist: company, company-server and company-launch.')
    for binary in binaries:
        validate_binary(binary, args.platform)
    archive = destination.parent / (destination.name + ('.zip' if extension else '.tar.gz'))
    if archive.exists():
        parser.error('The release archive already exists.')
    assets = public_assets()
    launcher = ROOT / ('Start-Company.ps1' if extension else 'Start-Company.sh')
    regular_file(launcher)
    assets.append(launcher)
    if extension and any(b'Start-Company.exe' in source.read_bytes()
                         for source in assets if source.suffix == '.md'):
        raise RuntimeError('Windows startup instructions must name company-launch.exe.')
    destination.mkdir(parents=True)
    for binary in binaries:
        shutil.copy2(binary, destination / binary.name)
    for source in assets:
        target = destination / source.relative_to(ROOT)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
    executable_names = {'company', 'company-server', 'company-launch', 'Start-Company.sh'}
    for name in executable_names:
        file = destination / name
        if file.exists():
            file.chmod(0o755)
    manifest = {
        'format': 'transplane-company-release', 'version': args.version,
        'platform': args.platform, 'rust_target': PLATFORMS[args.platform],
        'files': {
            file.relative_to(destination).as_posix(): hashlib.sha256(file.read_bytes()).hexdigest()
            for file in sorted(destination.rglob('*')) if file.is_file()
        },
    }
    (destination / 'release-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    if extension:
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as bundle:
            for file in sorted(destination.rglob('*')):
                if file.is_file():
                    bundle.write(file, destination.name + '/' + file.relative_to(destination).as_posix())
    else:
        with tarfile.open(archive, 'w:gz') as bundle:
            for file in sorted(destination.rglob('*')):
                if not file.is_file():
                    continue
                info = bundle.gettarinfo(file, arcname=destination.name + '/' + file.relative_to(destination).as_posix())
                info.mode = 0o755 if file.parent == destination and file.name in executable_names else 0o644
                info.uid = info.gid = 0
                info.uname = info.gname = ''
                with file.open('rb') as content:
                    bundle.addfile(info, content)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum = archive.parent / (archive.name + '.sha256')
    checksum.write_text(digest + '  ' + archive.name + '\n', encoding='utf-8')
    print(json.dumps({'archive': str(archive), 'sha256': digest, 'platform': args.platform}))


if __name__ == '__main__':
    main()
