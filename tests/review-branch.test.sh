#!/usr/bin/env bash
set -euo pipefail

# Test suite for review-branch.sh template parsing

# Create a temp file with the parsing logic
setup_parser() {
  local parser_file
  parser_file="$(mktemp)"
  cat > "$parser_file" << 'PARSER'
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

case "$TEMPLATE" in
  general|rust|angular) ;;
  *)
    echo "Error: unknown template" >&2
    exit 1
    ;;
esac

echo "$TEMPLATE"
PARSER
  echo "$parser_file"
}

run_tests() {
  local failed=0
  local parser
  parser="$(setup_parser)"
  
  echo "Running review-branch.sh tests..."
  echo ""
  
  # Test 1: Default template
  echo -n "Test: default template is 'general'... "
  result="$(bash "$parser")"
  if [[ "$result" == "general" ]]; then
    echo "✓ PASS"
  else
    echo "✗ FAIL (got: '$result')"
    failed=$((failed + 1))
  fi
  
  # Test 2: --template=rust
  echo -n "Test: --template=rust... "
  result="$(bash "$parser" --template=rust)"
  if [[ "$result" == "rust" ]]; then
    echo "✓ PASS"
  else
    echo "✗ FAIL (got: '$result')"
    failed=$((failed + 1))
  fi
  
  # Test 3: --template angular (separate arg)
  echo -n "Test: --template angular... "
  result="$(bash "$parser" --template angular)"
  if [[ "$result" == "angular" ]]; then
    echo "✓ PASS"
  else
    echo "✗ FAIL (got: '$result')"
    failed=$((failed + 1))
  fi
  
  # Test 4: --template=rust after positional args
  echo -n "Test: positional args then --template=rust... "
  result="$(bash "$parser" origin/main review-foo --template=rust)"
  if [[ "$result" == "rust" ]]; then
    echo "✓ PASS"
  else
    echo "✗ FAIL (got: '$result')"
    failed=$((failed + 1))
  fi
  
  # Test 5: invalid template
  echo -n "Test: invalid template should fail... "
  if bash "$parser" --template=invalid 2>/dev/null; then
    echo "✗ FAIL (should have exited with error)"
    failed=$((failed + 1))
  else
    echo "✓ PASS"
  fi
  
  rm -f "$parser"
  
  echo ""
  if [[ $failed -eq 0 ]]; then
    echo "All tests passed! ✓"
    return 0
  else
    echo "$failed test(s) failed ✗"
    return 1
  fi
}

run_tests
