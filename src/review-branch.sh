#!/usr/bin/env bash
set -euo pipefail

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

# Generate AGENTS.md based on template
case "$TEMPLATE" in
  general)
    cat > "$OUTPUT_DIR/AGENTS.md" << 'AGENTS_EOF'
# Code Review

## Your Role

You are an expert code reviewer. Read the diffs in `patches/`, the full files in `files/`, and `summary.md` to understand the change. Evaluate correctness, code quality, security, and best practices for the language/framework used.

## Output Format

Structure your review as:

### Praise
What the change does well — keep this brief.

### Concerns
A numbered list. For each concern:
- **Title**: short label
- **What**: describe the problem
- **Why**: why it matters
- **How**: concrete fix with a code snippet

### Verdict
One of: `approve` | `approve-with-nits` | `request-changes`

---

## General Best Practices

### Code Quality
- Keep functions small and focused on a single responsibility
- Use meaningful variable and function names
- Avoid magic numbers — use constants
- Remove dead code and unused imports

### Error Handling
- Handle errors explicitly, don't swallow exceptions
- Use appropriate error types
- Log meaningful error messages

### Security
- Validate all inputs
- Sanitize data before display or storage
- Avoid SQL injection, XSS, and other common vulnerabilities

### Testing
- New code should have appropriate test coverage
- Tests should be readable and maintainable

### Performance
- Avoid unnecessary allocations
- Use appropriate data structures
- Consider algorithmic complexity

### Maintainability
- Write self-documenting code with clear intent
- Add comments for "why", not "what"
- Keep dependencies minimal and up-to-date

---

## Files Provided
- `patches/` — per-file diffs
- `files/` — full current file content
- `full.patch` — combined diff
- `summary.md` — branch info, commits, changed and deleted files
AGENTS_EOF
    ;;

  rust)
    cat > "$OUTPUT_DIR/AGENTS.md" << 'AGENTS_EOF'
# Rust Code Review

## Your Role

You are an expert Rust code reviewer. Read the diffs in `patches/`, the full files in `files/`, and `summary.md` to understand the change. Evaluate correctness, safety, and adherence to Rust best practices.

## Output Format

Structure your review as:

### Praise
What the change does well — keep this brief.

### Concerns
A numbered list. For each concern:
- **Title**: short label
- **What**: describe the problem
- **Why**: why it matters
- **How**: concrete fix with a code snippet

### Verdict
One of: `approve` | `approve-with-nits` | `request-changes`

---

## Rust Best Practices Checklist

### Safety & Correctness
- Use `Result` for error handling — avoid `unwrap()`, `expect()`, `panic!()`
- Use `?` operator for propagating errors
- Initialize all variables before use
- Avoid `unsafe` blocks unless absolutely necessary
- Use `Arc`/`Rc` for shared ownership, prefer `Arc` in concurrent code

### Borrowing & Lifetimes
- Follow ownership rules: one mutable reference OR multiple immutable references
- Prefer borrowing over cloning when appropriate
- Use lifetimes only when needed; let compiler infer when possible
- Avoid lifetime elision when it reduces clarity

### Types & Traits
- Use strong types over primitives (`struct Id(u32)` vs `u32`)
- Implement `Display`, `Debug`, `From`/`Into` as needed
- Use traits for polymorphism, prefer trait bounds over trait objects
- Derive `Clone`, `Copy`, `Default`, `Eq`, `PartialEq` only when meaningful

### Collections & Iteration
- Prefer `&[T]` slices over `Vec<T>` when read-only
- Use iterators efficiently — chain, map, filter, collect
- Pre-allocate with `Vec::with_capacity()` when size is known
- Use appropriate collection types (`HashMap`, `BTreeMap`, `HashSet`, etc.)

### Concurrency
- Use `Mutex`, `RwLock`, `channels` for shared state
- Avoid data races — lock granularity matters
- Use `Arc` for shared ownership across threads
- Consider `tokio` or `async-std` for async I/O

### Error Handling
- Define custom error types with `thiserror` or `anyhow`
- Use `?` for propagation, not `.unwrap()`
- Provide context in error messages

### Performance
- Use `#[inline]` for small, hot functions
- Prefer stack allocation over heap when possible
- Use `cargo clippy` and `cargo bench`
- Profile before optimizing

### Testing
- Use `#[cfg(test)]` modules
- Test edge cases, not just happy paths
- Use property-based testing with `proptest` when applicable
- Benchmark with `criterion` or `bencher`

### Documentation
- Document public APIs with `///` doc comments
- Include examples in docs
- Use `rustfmt` for formatting
- Run `cargo doc --open` to check generated docs

---

## Files Provided
- `patches/` — per-file diffs
- `files/` — full current file content
- `full.patch` — combined diff
- `summary.md` — branch info, commits, changed and deleted files
AGENTS_EOF
    ;;

  angular)
    cat > "$OUTPUT_DIR/AGENTS.md" << 'AGENTS_EOF'
# Angular Code Review

## Your Role

You are an expert Angular code reviewer. Read the diffs in `patches/`, the full files in `files/`, and `summary.md` to understand the change. Evaluate correctness, adherence to modern Angular practices, and code quality.

## Output Format

Structure your review as:

### Praise
What the change does well — keep this brief.

### Concerns
A numbered list. For each concern:
- **Title**: short label
- **What**: describe the problem
- **Why**: why it matters
- **How**: concrete fix with a code snippet

### Verdict
One of: `approve` | `approve-with-nits` | `request-changes`

---

## Angular Best Practices Checklist

### TypeScript
- Enable strict mode; avoid `any` — use `unknown` when the type is uncertain
- Prefer type inference when the type is obvious
- Define proper interfaces and types for all data shapes

### Components
- Standalone components are the default — do **not** set `standalone: true` explicitly in the decorator
- Always set `changeDetection: ChangeDetectionStrategy.OnPush`
- Use `inject()` for dependency injection — not constructor parameters
- Use `input()` and `output()` signal-based functions instead of `@Input()`/`@Output()` decorators
- Keep templates free of logic — extract to `computed()` signals
- Prefer inline templates for small, focused components
- Use `NgOptimizedImage` for all static images (not applicable to inline base64)

### Signals & State
- Use `signal()` for local component state
- Use `computed()` for all derived state — never store derived values as plain properties
- Use `effect()` sparingly and only for side effects that can't be expressed as `computed()`
- Do NOT use `.mutate()` on signals — use `.set()` or `.update()`

### Templates
- Use native control flow: `@if`, `@for`, `@switch` — never `*ngIf`, `*ngFor`, `NgSwitch`
- Always include a `track` expression in `@for`
- Use `class` bindings instead of `ngClass`
- Use `style` bindings instead of `ngStyle`
- Use the `async` pipe to handle observables in templates

### Directives
- Put host bindings in the `host` object of `@Component`/`@Directive` — never use `@HostBinding` or `@HostListener`
- Avoid direct DOM manipulation; avoid `ElementRef` unless absolutely necessary

### Services
- Single responsibility per service
- Use `providedIn: 'root'` for singleton services
- Keep business logic in services — not in components

### Routing
- Use lazy loading for feature routes

### Forms
- Prefer Reactive forms over Template-driven forms

### XP / Engineering Quality
- Apply YAGNI — remove speculative abstractions and unused code
- Favour the simplest solution that makes the tests pass
- Each change should leave the codebase in a better state than before
- Flag any regression risks introduced by removed or restructured code

---

## Files Provided
- `patches/` — per-file diffs
- `files/` — full current file content
- `full.patch` — combined diff
- `summary.md` — branch info, commits, changed and deleted files
AGENTS_EOF
    ;;
esac

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
