## Summary

<!-- Briefly describe what this Pull Request adds, fixes, or improves. -->

## Related Issue(s)

<!-- Link any related issues, e.g. Closes #12 -->

## Changes Made

- 

## Safety, Privacy & Verification Checklist

- [ ] `cargo test --workspace` passes across all subsystems
- [ ] `npm run lint:rust` (`cargo clippy -- -D warnings`) and `npm run fmt:rust` pass with zero warnings
- [ ] `python3 scripts/generate_release_integrity.py` has been run (if tracked files were modified)
- [ ] **Zero Network Egress**: Introduces no network sockets, remote assets, or banned networking crates
- [ ] **Failsafe Compliance**: Any audio or display changes respect limiter/gamma bounds and are immediately cleared by the Triple-`ESC` Exorcise failsafe
