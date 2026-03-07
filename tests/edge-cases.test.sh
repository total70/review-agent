#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=helpers.sh
source "$SCRIPT_DIR/helpers.sh"

main() {
  eval "$(setup_repo)"
  WORK_DIR="$WORK_DIR"

  # Edge: no changes on branch
  repo_create_branch "$WORK_DIR" feature/no-changes
  repo_push_current "$WORK_DIR"

  pushd "$WORK_DIR" >/dev/null
  OUTDIR="review-feature-no-changes"
  bash "$SCRIPT_PATH" origin/master "$OUTDIR" --template=general > /dev/null 2>&1 || {
    echo "ASSERT FAIL: script should succeed when no changes" >&2; exit 1;
  }
  assert_file_exists "$WORK_DIR/$OUTDIR/summary.md"
  assert_contains "$WORK_DIR/$OUTDIR/summary.md" "## Changed Files"
  assert_contains "$WORK_DIR/$OUTDIR/summary.md" "_None_"
  assert_contains "$WORK_DIR/$OUTDIR/summary.md" "## Deleted Files"
  assert_contains "$WORK_DIR/$OUTDIR/summary.md" "_None_"

  # Edge: empty repo beyond initial commit (already set). Create brand new repo without commits
  tmp="$(mktemp -d)"
  pushd "$tmp" >/dev/null
  git init -q
  _git_config_repo || true
  # No commits yet; running the script should fail because merge-base/log/diff cannot run without commits
  if bash "$SCRIPT_PATH" 2>err.log; then
    echo "ASSERT FAIL: script should fail in repo with no commits" >&2
    exit 1
  fi
  [[ -s err.log ]] || { echo "ASSERT FAIL: expected error output for empty repo" >&2; exit 1; }
  popd >/dev/null

  popd >/dev/null
}

main "$@"
