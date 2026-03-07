#!/usr/bin/env bash
set -euo pipefail

# Helper utilities for testing review-branch.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SCRIPT_PATH="$REPO_ROOT/scripts/review-branch.sh"

# Create shim tools to normalize behavior across macOS/Linux (e.g., install -D)
_setup_shims() {
  SHIM_DIR="$(mktemp -d)"
  cat > "$SHIM_DIR/install" <<'SHIM'
#!/usr/bin/env bash
set -euo pipefail
# Minimal shim for `install` supporting GNU -D flag used by the script
if [[ "${1:-}" == "-D" && $# -eq 3 ]]; then
  src="$2"; dest="$3"; dir="$(dirname "$dest")"; mkdir -p "$dir"; cp -f "$src" "$dest"
  exit 0
fi
# Prefer ginstall if present (coreutils)
if command -v ginstall >/dev/null 2>&1; then
  exec ginstall "$@"
fi
# Fallback to system install
exec /usr/bin/install "$@"
SHIM
  chmod +x "$SHIM_DIR/install"
  export PATH="$SHIM_DIR:$PATH"
}

_setup_shims

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }
}

require_cmd git

mktempdir() {
  local d
  d="$(mktemp -d)"
  echo "$d"
}

# Create a git user for tests
_git_config_repo() {
  git config user.name "Test User"
  git config user.email "test@example.com"
}

# Create a fresh origin (bare) and working clone with an initial commit on master
# Outputs:
# - Echoes the workdir path
# - Sets ORIGIN_DIR and WORK_DIR env vars for the caller (use eval "$(setup_repo)")
setup_repo() {
  local tmp base origin work
  tmp="$(mktempdir)"
  base="$tmp/base"
  origin="$tmp/origin.git"
  work="$tmp/work"

  mkdir -p "$base" && pushd "$base" >/dev/null
  git init -q
  _git_config_repo
  echo "initial" > README.md
  git add README.md
  git commit -q -m "chore: initial"
  git branch -M master
  git init -q --bare "$origin"
  # Ensure bare repo HEAD points to master to avoid clone warnings
  git --git-dir="$origin" symbolic-ref HEAD refs/heads/master >/dev/null 2>&1 || true
  git remote add origin "$origin"
  git push -q -u origin master
  popd >/dev/null

  git clone -q "$origin" "$work"
  pushd "$work" >/dev/null
  _git_config_repo
  echo "WORK_DIR=$work"
  echo "ORIGIN_DIR=$origin"
  popd >/dev/null
}

# Create a feature branch from master and apply a set of changes
# Usage: repo_apply_changes WORK_DIR BRANCH
# The function assumes master exists and is pushed to origin
repo_create_branch() {
  local workdir="$1" branch="$2"
  pushd "$workdir" >/dev/null
  git checkout -q -b "$branch" origin/master
  popd >/dev/null
}

# Convenience commit helpers
repo_add_file() {
  local workdir="$1" path="$2" content="$3"
  pushd "$workdir" >/dev/null
  mkdir -p "$(dirname "$path")"
  printf "%s" "$content" > "$path"
  git add "$path"
  git commit -q -m "feat: add $path"
  popd >/dev/null
}

repo_modify_file() {
  local workdir="$1" path="$2" content_append="$3"
  pushd "$workdir" >/dev/null
  printf "%s" "$content_append" >> "$path"
  git add "$path"
  git commit -q -m "chore: modify $path"
  popd >/dev/null
}

repo_delete_file() {
  local workdir="$1" path="$2"
  pushd "$workdir" >/dev/null
  git rm -q "$path"
  git commit -q -m "refactor: delete $path"
  popd >/dev/null
}

repo_push_current() {
  local workdir="$1"
  pushd "$workdir" >/dev/null
  local cur
  cur="$(git rev-parse --abbrev-ref HEAD)"
  git push -q -u origin "$cur"
  popd >/dev/null
}

# Run the review script inside WORK_DIR with given args
run_review() {
  local workdir="$1"; shift
  pushd "$workdir" >/dev/null
  bash "$SCRIPT_PATH" "$@"
  popd >/dev/null
}

# Simple assertion helpers
assert_file_exists() { [[ -f "$1" ]] || { echo "ASSERT FAIL: expected file exists: $1" >&2; return 1; }; }
assert_dir_exists() { [[ -d "$1" ]] || { echo "ASSERT FAIL: expected dir exists: $1" >&2; return 1; }; }
assert_contains() { grep -qE -- "$2" "$1" || { echo "ASSERT FAIL: expected '$1' to contain /$2/" >&2; return 1; }; }
assert_not_contains() { if grep -qE -- "$2" "$1"; then echo "ASSERT FAIL: expected '$1' NOT to contain /$2/" >&2; return 1; fi }
assert_empty_file() { [[ ! -s "$1" ]] || { echo "ASSERT FAIL: expected empty file: $1" >&2; return 1; }; }
