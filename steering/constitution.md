# Project Constitution

## Purpose

Build the Rust game core in a way that is technically sound and helps the developer learn how each part works. Prefer small, understandable steps over large generated solutions.

## Architecture boundaries

- The Rust core owns game rules and simulation. Hosts own rendering, hardware input, audio, and menus.
- Keep gameplay headless and engine-independent. Do not add renderer dependencies to core gameplay crates.
- Keep Rapier behind the physics API. Do not expose Rapier types in the game's public API or saved data.
- Use stable entity IDs, fixed simulation ticks, validated inputs, and explicit errors at external boundaries.
- Follow the architecture and milestone order in `docs/Rust_Game_Core_Architecture.md`. Explain proposed deviations before making them.
- Do not add multiplayer, networking, extra processes, or speculative frameworks during Stage 1.

## Learning is part of the deliverable

- Separate advice from action. If the user asks what to do, how to start, or asks only to plan, compare options, or understand a concept, give clear next steps and stop; do not edit files or run setup commands.
- Before any file edit or state-changing project command, explain in plain language exactly what will change or run, why, and how it will be checked. Then wait for the user's explicit go-ahead.
- Do not treat general permission to use the terminal, an earlier request, or the act of asking for a plan as approval for a specific action. If the user wants to do the work themselves, provide commands and explanations without executing them.
- After approval, do only the described work. Ask again before changing files or running commands outside that scope. Include intended verification commands in the preview so the user knows what the approval covers.
- Make explanations actionable for a learner: identify exact file paths, summarize the proposed code changes, give literal terminal commands when useful, explain each command's purpose, and state the expected result. Clearly label which steps are for the user to perform and which actions the agent proposes to perform.
- Make the smallest useful change and explain unfamiliar concepts using the code being worked on.
- After editing, run the narrowest useful check first. Report what it proves and any remaining gaps.
- Summaries should say what changed, what was learned, and what remains unfinished.

## Code and comments

- Prefer clear Rust structs, enums, functions, and traits at meaningful boundaries. Do not force object-oriented inheritance or purely functional patterns.
- Write useful comments in simple language. Explain why non-obvious choices exist and how multi-step logic works; use Rust documentation comments for public APIs where useful.
- Do not comment every line or merely translate syntax into English. Comments should help a learner understand purpose, flow, or a trade-off.
- Avoid unexplained magic numbers and unnecessary abstractions. Keep unsafe code isolated and explain its safety requirements.
- Never use `.unwrap()` or `.expect()` on untrusted input or public FFI paths.

## Commands and verification

- Prefer established tools for the work they are designed to do: Cargo for scaffolding, dependencies, formatting, tests, linting, and builds.
- Use commands for project operations, not shell tricks to generate or overwrite Rust source code.
- Check the pinned tool and dependency versions before relying on an API or command option.
- Never claim a test or check passed unless it was actually run.

## Inline-friendly collaboration

- Keep inline changes local to the active function or small code region. Avoid unrelated cleanup and broad rewrites.
- Use nearby names, tests, and short comments to make intent clear to completion tools.
- Inline completions do not reliably load repository instructions in every product. Put essential context in the open code or the user's inline request; use an agent/chat workflow when a change needs broader reasoning.
