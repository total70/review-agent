# AGENTS.md - Review Agent

## Code Quality Rules

**Always run these before considering implementation complete:**

```bash
# Run the check script (fmt + clippy + tests)
./scripts/check.sh

# Or run individually:
cargo check
cargo build
cargo test
```

## Project Structure

- `src/main.rs` - Entry point, command dispatching
- `src/cli.rs` - CLI argument parsing
- `src/pack.rs` - Git packing logic
- `src/review.rs` - Review execution logic
- `src/html.rs` - HTML rendering
- `src/providers/` - AI provider integrations
