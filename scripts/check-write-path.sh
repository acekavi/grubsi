#!/usr/bin/env bash
# Fails if a database transaction is opened anywhere but `write_tx`.
#
# `write_tx` makes the audit record a required argument. That guarantee
# only holds while it is the sole place a transaction begins — the write
# pool is still public (making it private is M1's job), so this stands in
# for the compiler until then.
set -euo pipefail

ALLOWED="crates/server/src/infra/write.rs"

offenders="$(grep -rn --include='*.rs' -F '.begin(' crates/ \
             | grep -v "^${ALLOWED}:" || true)"

if [ -n "$offenders" ]; then
  echo "Transaction opened outside ${ALLOWED}:" >&2
  echo "$offenders" >&2
  echo "" >&2
  echo "Every write goes through infra::write::write_tx, which records an" >&2
  echo "audit entry in the same transaction. See the spec, section 2.3." >&2
  exit 1
fi
echo "write path boundary OK"
