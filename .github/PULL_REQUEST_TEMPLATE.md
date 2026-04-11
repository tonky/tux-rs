## Description

Please include a summary of the change and which issue is fixed. Please also include relevant motivation and context. Laptops verified with this PR:

Fixes # (issue)

## Type of change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] New hardware support (adding a new laptop model/SKU)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update

## Checklist

Before submitting, ensure that your PR passes all required checks:

- [ ] My code follows the Rust styling guidelines of this project
- [ ] I have executed `just check`
- [ ] `cargo test` passes locally with my changes
- [ ] `cargo clippy` emits no new warnings
- [ ] I have commented my code, particularly in hard-to-understand areas (`tux-core` / `dbus` / `kernel shims`)
- [ ] I have made corresponding changes to the documentation (if applicable)

If adding hardware support:
- [ ] I have cross-referenced the ACPI/WMI/EC methods safely.
