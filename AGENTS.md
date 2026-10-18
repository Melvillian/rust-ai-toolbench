# Agent instructions

`CLAUDE.md` is the shared source of truth for repository instructions, regardless
of which agent or harness you use. Read and follow [CLAUDE.md](CLAUDE.md) before
working in this repository.

When working in a subdirectory, also read any `CLAUDE.md` files along the path
from the repository root to that directory. More specific instructions apply
within their directory and its descendants and take precedence over broader
repository instructions.

Keep shared guidance in the appropriate `CLAUDE.md` rather than duplicating it
here or creating separate harness-specific versions. This file is the entry
point for Codex and other harnesses that discover `AGENTS.md`; Claude continues
to use the same `CLAUDE.md` files directly.
