#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
target=""
skip_web=false
while (($#)); do
  case "$1" in
    --target) target="${2:?Specify a Rust target}"; shift 2 ;;
    --skip-web) skip_web=true; shift ;;
    *) printf 'Unknown build option: %s\n' "$1" >&2; exit 2 ;;
  esac
done
cd -- "$project_root"
python3 - "$project_root" "$target" <<'PY'
import shutil, sys
from pathlib import Path
root = Path(sys.argv[1]).resolve()
generated = root / 'target'
if generated.is_symlink():
    raise SystemExit('Refusing to clean a redirected build directory.')
parents = [generated / 'debug', generated / 'release']
if sys.argv[2]:
    parents += [generated / sys.argv[2] / 'debug', generated / sys.argv[2] / 'release']
for parent in parents:
    path = parent / 'incremental'
    if not path.exists():
        continue
    if not path.resolve().is_relative_to(generated.resolve()) or path.is_symlink() or any(p.is_symlink() for p in path.rglob('*')):
        raise SystemExit('Refusing to clean redirected build artifacts.')
    shutil.rmtree(path)
PY
if [[ "$skip_web" == false ]]; then
  (cd web && npm ci && npm run build)
fi
arguments=(build --locked --release --bins)
if [[ -n "$target" ]]; then arguments+=(--target "$target"); fi
cargo "${arguments[@]}"
