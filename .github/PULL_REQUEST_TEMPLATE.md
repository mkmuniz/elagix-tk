## What this changes

## Why

## Checklist
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] New filters/rules include a test with a real captured fixture (not synthetic)
- [ ] Still respects the business rules in `specs.md` §4 (especially rule 6: never inflate the output, and rule 3: fail-open)
