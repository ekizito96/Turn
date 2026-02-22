# Security Policy

## Supported Versions

| Version       | Supported          |
| ------------- | ------------------ |
| 0.5.0-alpha   | :white_check_mark: |
| 0.4.0 and below | :x:              |

## Security Model

Turn has a built-in **Object-Capability (OCap) Security Model** as of v0.5.0-alpha.

- **`Value::Cap`** is an opaque integer handle that references a Host-side secret (API key, DB connection, etc.). The actual credential never enters the Turn VM heap.
- Capabilities cannot be serialized, printed, or passed to `infer`. Any attempt to evaluate a capability in a guest expression raises a `PrivilegeViolation` trap immediately.
- The `secret` parameter modifier prevents sensitive parameters from being included in LLM tool schemas — the model never sees them.

These primitives are enforced by the VM and Rust type system, not by convention.

## Reporting a Vulnerability

If you discover a security vulnerability in Turn, please report it responsibly:

1. **Do not** open a public GitHub issue
2. Email the maintainer directly: **muyukaniephraim@yahoo.com**
3. Include:
   - A clear description of the vulnerability
   - Steps to reproduce
   - The version of Turn affected
   - Potential impact (e.g., capability leakage, sandbox escape)
4. Allow reasonable time for a fix before public disclosure

We will acknowledge your report within 48 hours and work to address it promptly. Security fixes are prioritized above all other work.

## Threat Model

Turn's VM is a **guest execution environment**. The threat model is:

- **Trusted**: The Rust Host (the process running the VM), capability registry, and tool implementations
- **Untrusted**: Turn scripts themselves, LLM-generated content, and user-supplied turn source code

The VM is designed to prevent guest scripts from leaking capability handles or executing unauthorized side effects outside the declared tool interface.
