#!/usr/bin/env bash
# Fails if grubsi-core gains an I/O dependency, directly or transitively.
# The spec's `core` boundary is only meaningful if it is enforced.
set -euo pipefail

FORBIDDEN=(tokio sqlx axum hyper tower reqwest sqlx-core sqlx-sqlite)

deps="$(cargo tree --package grubsi-core --edges normal --prefix none \
        | awk '{print $1}' | sort -u)"

fail=0
for f in "${FORBIDDEN[@]}"; do
  if grep -qx -- "$f" <<<"$deps"; then
    echo "FORBIDDEN dependency in grubsi-core: $f" >&2
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  echo "" >&2
  echo "crates/core must stay free of I/O. See the spec, section 3." >&2
  exit 1
fi
echo "core dependency boundary OK"
