#!/bin/sh
set -eu

case "$0" in
    */*) script_directory=${0%/*} ;;
    *) script_directory=. ;;
esac
script_directory=$(CDPATH= cd "$script_directory" && pwd -P)

python=${PYTHON:-python3}
case "$python" in
    /*) ;;
    *) python=$(command -v "$python") || {
        echo "benchmark launcher cannot find Python: $python" >&2
        exit 2
    } ;;
esac

exec env -i \
    GREATGRAMMA_BENCHMARK_SANITIZED=1 \
    HOME="${HOME:?benchmark launcher requires HOME}" \
    LANG=C \
    LC_ALL=C \
    PATH="${PATH:-/usr/local/bin:/usr/bin:/bin}" \
    TMPDIR="${TMPDIR:-/tmp}" \
    "$python" -I "$script_directory/benchmark-python.py" "$@"
