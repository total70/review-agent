#!/usr/bin/env bash
set -euo pipefail

# Get the directory where this script is located (for template paths)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Usage: review-branch.sh [base-branch] [output-dir] [--template=<template>]
#
# Arguments:
#   base-branch     : Base branch to compare against (default: origin/master)
#   output-dir      : Output directory for review files (default: review-<branch>)
#   --template=<>  : Agent template to use: general, rust, angular (default: general)
#
# Examples:
#   review-branch.sh
#   review-branch.sh origin/main my-review
#   review-branch.sh --template=rust
#   review-branch.sh origin/develop --template=angular

# Check for test mode first (before any git operations)
if [[ "${1:-}" == "--test" ]]; then
  run_tests
  exit $?
fi

# Parse arguments
TEMPLATE="general"
REMAINING_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --template=*)
      TEMPLATE="${1#*=}"
      shift
      ;;
    --template)
      TEMPLATE="$2"
      shift 2
      ;;
    *)
      REMAINING_ARGS+=("$1")
      shift
      ;;
  esac
done

# Validate template
case "$TEMPLATE" in
  general|rust|angular) ;;
  *)
    echo "Error: unknown template '$TEMPLATE'. Use: general, rust, or angular" >&2
    exit 1
    ;;
esac

# Set remaining args
set -- "${REMAINING_ARGS[@]}"

BASE_BRANCH="${1:-origin/master}"
BRANCH_NAME=$(git rev-parse --abbrev-ref HEAD)
OUTPUT_DIR="${2:-review-${BRANCH_NAME}}"

# Fail early if not in a git repo
if ! git rev-parse --is-inside-work-tree &>/dev/null; then
  echo "Error: not inside a git repository" >&2
  exit 1
fi

GIT_ROOT=$(git rev-parse --show-toplevel)
cd "$GIT_ROOT"

echo "Fetching origin..."
git fetch origin --quiet

MERGE_BASE=$(git merge-base "$BASE_BRANCH" HEAD)

CHANGED_FILES=$(git diff "$MERGE_BASE" HEAD --name-only --diff-filter=ACMRT 2>/dev/null || true)
DELETED_FILES=$(git diff "$MERGE_BASE" HEAD --name-only --diff-filter=D 2>/dev/null || true)

# Create output dirs
mkdir -p "$OUTPUT_DIR/patches" "$OUTPUT_DIR/files"

# Per-file patches and copies
FILE_COUNT=0
PATCH_COUNT=0

if [[ -n "$CHANGED_FILES" ]]; then
  while IFS= read -r file; do
    [[ -z "$file" ]] && continue

    # Create patch
    patch_path="$OUTPUT_DIR/patches/${file}.patch"
    mkdir -p "$(dirname "$patch_path")"
    git diff "$MERGE_BASE" HEAD -- "$file" > "$patch_path"
    PATCH_COUNT=$((PATCH_COUNT + 1))

    # Copy full file
    if [[ -f "$file" ]]; then
      mkdir -p "$OUTPUT_DIR/files/$(dirname "$file")"
      cp "$file" "$OUTPUT_DIR/files/$file"
      FILE_COUNT=$((FILE_COUNT + 1))
    fi
  done <<< "$CHANGED_FILES"
fi

# Full combined diff
git diff "$MERGE_BASE" HEAD > "$OUTPUT_DIR/full.patch"

# Commit log
COMMIT_LOG=$(git log "$MERGE_BASE"..HEAD --oneline)

# Generate summary.md
{
  echo "# Branch Review Summary"
  echo ""
  echo "**Branch:** \`$BRANCH_NAME\`"
  echo "**Base:** \`$BASE_BRANCH\`"
  echo "**Merge base:** \`$MERGE_BASE\`"
  echo ""
  echo "## Commits"
  echo '```'
  echo "$COMMIT_LOG"
  echo '```'
  echo ""
  echo "## Changed Files"
  if [[ -n "$CHANGED_FILES" ]]; then
    while IFS= read -r file; do
      [[ -z "$file" ]] && continue
      echo "- $file"
    done <<< "$CHANGED_FILES"
  else
    echo "_None_"
  fi
  echo ""
  echo "## Deleted Files"
  if [[ -n "$DELETED_FILES" ]]; then
    while IFS= read -r file; do
      [[ -z "$file" ]] && continue
      echo "- $file"
    done <<< "$DELETED_FILES"
  else
    echo "_None_"
  fi
} > "$OUTPUT_DIR/summary.md"

# Create empty AGENTS.md - will be filled in by Rust code after script completes
mkdir -p "$OUTPUT_DIR"
touch "$OUTPUT_DIR/AGENTS.md"

# Print summary
echo ""
echo "Review package created: $OUTPUT_DIR/"
echo "  Template             : $TEMPLATE"
echo "  Changed files copied: $FILE_COUNT"
echo "  Patch files created  : $PATCH_COUNT"
if [[ -n "$DELETED_FILES" ]]; then
  DELETED_COUNT=$(echo "$DELETED_FILES" | grep -c . || true)
  echo "  Deleted files noted  : $DELETED_COUNT (see summary.md)"
fi
echo ""
echo "To share: zip -r review-${BRANCH_NAME}.zip $OUTPUT_DIR/"
