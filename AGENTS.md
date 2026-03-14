# AGENTS.md

## Skills

Available local skills for this repo:

- `release`: Cut a release for markless. Runs checks, bumps version, commits and pushes, merges the PR, tags, and publishes to crates.io. File: `/workspace/.claude/skills/release/SKILL.md`

## How To Use Skills

- If the user asks for a release workflow, use the `release` skill.
- Read the referenced `SKILL.md` and follow it directly.
- Prefer the skill instructions over ad hoc release steps.
