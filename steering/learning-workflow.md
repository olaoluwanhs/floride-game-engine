# Learning Workflow

## Advice, approval, and action

- Treat asking for advice and asking for implementation as different things. If the user asks what to do, how to start, or only asks to plan or learn, give the recommended steps and stop. Do not edit files or run setup, install, update, or other state-changing commands.
- Before any file edit or state-changing project command, explain the exact files or commands involved, their expected effect, and the check you intend to run. Then stop and wait for the user's explicit go-ahead.
- A general statement that terminal commands are allowed is not approval for every command. An earlier approval does not cover a new or expanded action. If the user wants to do the work themselves, provide the commands and explain them, but do not execute them.
- Once the user approves, perform only the described scope. Pause for another go-ahead if the work needs a materially different edit or additional state-changing command.
- Make the explanation usable by someone learning the project: name the exact files, briefly summarize what code or text will change, show literal commands when they help, explain each command's purpose, and say what successful output or behavior to expect. Clearly separate user-run steps from agent-run steps.

## Before implementation

Explain briefly, in everyday language:

- What behavior or project piece will change.
- Which exact files will be created or changed, and the main code changes in each.
- Which commands will be run, what they do, and what success should look like.
- Why this is the right place for the change.
- Which small test, command, or observation can show whether it worked.
- Which Rust idea is useful to understand for this step.

Do not edit until the user has approved the proposed implementation after hearing this explanation. If the task is unclear or conflicts with the architecture, explain the issue and ask a focused question instead of guessing.

## While implementing

- Work in small changes that can be understood and checked independently.
- Treat the developer as experienced with TypeScript and new to Rust. Explain unfamiliar Rust concepts such as ownership, borrowing, traits, generics, and lifetimes in the context where they appear. Use TypeScript comparisons when helpful, and mention where the comparison stops being accurate.
- Use descriptive names and show the steps in the logic. Prefer named methods and intermediate variables over operator overloading, dense expressions, or long method chains when those forms make the behavior less clear.
- Prefer existing project patterns and standard commands. Explain a command's purpose when it is unfamiliar; do not replace a simple Cargo command with generated code.
- For inline suggestions, provide a focused expression, branch, method, or test. Keep nearby context intact and avoid rewriting unrelated code.
- Add comments where they explain a decision, domain rule, or Rust concept that is not clear from the code. Public APIs may need `///` documentation. Do not narrate every obvious line or use comments as a substitute for clear names.
- When the behavior is not yet known, state a small hypothesis and use a focused check to test it.

## After a change

- Run the cheapest relevant check immediately after the first edit: a focused test, then a narrow build/lint check as appropriate.
- If it fails, explain the result plainly and fix the same small area before broadening the investigation.
- Summarize the behavior, the verification result, and the key idea learned. Be clear about tests that could not be run.

## Inline completion notes

Ghost-text completion may rely more on the current file and nearby context than on repository documents. Before requesting a suggestion, a short nearby comment can state the goal and constraints. Review generated code for correctness, architecture fit, and tests; an accepted suggestion is not verification.
