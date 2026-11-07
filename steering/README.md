# Shared Steering Documents

These Markdown files are the provider-neutral home for project direction. They are written for people and AI coding tools, not for one vendor or IDE.

## Start here

1. Read the repository-root `AGENTS.md` for the entry points.
2. Read `constitution.md` for project boundaries and non-negotiable rules.
3. Read `learning-workflow.md` before planning or implementing a feature.
4. Read `technical-baseline.md` for the agreed Rust, library, game-loop, and threading approach.
5. Use `../docs/Rust_Game_Core_Architecture.md` for the complete technical design and milestone acceptance criteria.

## Approval before action

When the user asks what to do or requests a plan, provide the next steps and stop. Before editing files or running state-changing project commands, describe the exact action and intended check, then wait for the user's explicit go-ahead. See `learning-workflow.md` for the full rule.

Make those next steps concrete for a learner: include exact paths, a short change summary, useful terminal commands with their purpose, and the expected result. Clearly state whether the user or the agent is expected to perform each step.

## Using these with different tools

`AGENTS.md` is a common repository instruction filename, but no one folder or filename is automatically discovered by every AI product. Tools that support `AGENTS.md` can use the root guide directly. In other tools, add `AGENTS.md` or this `steering/` directory as project context, or ask the assistant to read these documents. Do not create a vendor-specific copy of the steering rules; keep this folder as the shared source and use only short discovery pointers when a tool requires them.

The existing `.github/` instruction files are lightweight Copilot discovery pointers. They should not become a second source of project policy.
