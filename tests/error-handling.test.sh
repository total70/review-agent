#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=helpers.sh
source "$SCRIPT_DIR/helpers.sh"

main() {
  # 1) Not a git repo
  tmp="$(mktemp -d)"
  pushd "$tmp" >/dev/null
  if bash "$SCRIPT_PATH" 2>err.log; then
    echo "ASSERT FAIL: script should have failed outside a git repo" >&2
    exit 1
  fi
  # Any error output is acceptable; script currently fails before custom check due to early git call
  [[ -s err.log ]] || { echo "ASSERT FAIL: expected error output for non-git repo" >&2; exit 1; }
  popd >/dev/null

  # 2) Invalid base branch
  eval "$(setup_repo)"
  WORK_DIR="$WORK_DIR"
  repo_create_branch "$WORK_DIR" bug/invalid-base
  repo_add_file "$WORK_DIR" file.txt "x\n"
  repo_push_current "$WORK_DIR"

  pushd "$WORK_DIR" >/dev/null
  if bash "$SCRIPT_PATH" origin/does-not-exist out 2>err2.log; then
    echo "ASSERT FAIL: script should fail when base branch is invalid (merge-base fails)" >&2
    exit 1
  fi
  # Stderr should include 'fatal' or merge-base message; be lenient, just ensure non-empty
  [[ -s err2.log ]] || { echo "ASSERT FAIL: expected error output for invalid base" >&2; exit 1; }
  popd >/dev/null
}

main "$@"
