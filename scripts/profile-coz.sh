#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != Linux ]]; then
    printf '%s\n' 'Coz profiling is supported only on Linux hosts.' >&2
    exit 2
fi
if ! command -v coz >/dev/null 2>&1; then
    printf '%s\n' 'Coz is unavailable; run this script inside `nix develop` on Linux.' >&2
    exit 2
fi
if (($# == 0)); then
    printf 'Usage: %s <cargo-bin-target> [workload arguments...]\n' "$0" >&2
    exit 2
fi

target="$1"
shift
cargo build --profile coz --workspace

profile="${COZ_PROFILE:-${target}.coz}"
printf 'Coz profile: %s\n' "$profile"
binary="target/coz/$target"
if [[ ! -x "$binary" ]]; then
    printf 'Cargo binary target not found: %s\n' "$binary" >&2
    exit 2
fi
coz run --binary-scope MAIN --output "$profile" --- "$binary" "$@"
