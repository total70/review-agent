#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=helpers.sh
source "$SCRIPT_DIR/helpers.sh"

main() {
  eval "$(setup_repo)"
  WORK_DIR="$WORK_DIR"

  repo_create_branch "$WORK_DIR" feature/agents
  repo_add_file "$WORK_DIR" src/a.rs "fn main(){}\n"
  repo_push_current "$WORK_DIR"

  pushd "$WORK_DIR" >/dev/null

  # rust template
  OUT1="review-feature-agents-rust"
  bash "$SCRIPT_PATH" origin/master "$OUT1" --template=rust > /dev/null 2>&1
  assert_file_exists "$WORK_DIR/$OUT1/AGENTS.md"
  assert_contains "$WORK_DIR/$OUT1/AGENTS.md" "# Rust Code Review"
  assert_contains "$WORK_DIR/$OUT1/AGENTS.md" "Rust Best Practices Checklist"

  # angular template
  OUT2="review-feature-agents-angular"
  bash "$SCRIPT_PATH" origin/master "$OUT2" --template=angular > /dev/null 2>&1
  assert_file_exists "$WORK_DIR/$OUT2/AGENTS.md"
  assert_contains "$WORK_DIR/$OUT2/AGENTS.md" "# Angular Code Review"
  assert_contains "$WORK_DIR/$OUT2/AGENTS.md" "Angular Best Practices Checklist"

  popd >/dev/null
}

main "$@"
