#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_DIR="$ROOT_DIR/tests"

chmod +x "$TEST_DIR"/*.sh || true

failed=0

echo "Running tests..."

for t in "$TEST_DIR"/*.test.sh "$TEST_DIR"/cases/*.sh; do
  [[ -f "$t" ]] || continue
  echo "-- $t"
  if bash "$t"; then
    echo "OK"
  else
    echo "FAIL: $t"
    failed=$((failed+1))
  fi
  echo
done

if [[ $failed -gt 0 ]]; then
  echo "$failed test(s) failed"
  exit 1
fi

echo "All tests passed"
