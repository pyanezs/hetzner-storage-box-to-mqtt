# CLAUDE.md

## General
- NEVER use emojis

## Style

- Use semantic line breaks for all documentation and comments:
  start each sentence or major clause on its own line
  instead of wrapping at a fixed column width.

## Git

- Always ask for user approval before running `git commit`, `git push`,
  or any destructive git commands.
- Never push without explicit user confirmation.
- Do not add Claude to the commits authors.
- Always call `mise prek-run` before creating a commit.
  If hooks fail, you should not create a commit.
- NEVER add yourself as a co-author
- Keep commits simple. Use a title, and list changes in
  short phrases using bullet points

  ## Mise

- Use the MCP to run mise tasks.

## Logging

- Always use struct logging!
- Log lines always start with upper case
