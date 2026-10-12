#!/usr/bin/env sh
set -eu
task_install_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$task_install_dir/company-launch" "$@"
