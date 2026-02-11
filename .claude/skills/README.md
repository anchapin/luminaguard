# Claude Code Skills

This directory contains custom skills for Claude Code (claude.ai/code).

## Available Skills

### `/pr-swarmit`

Parallel PR investigation and fixing using git worktrees and sub-agents.

**Documentation**: See [`pr-swarmit/SKILL.md`](./pr-swarmit/SKILL.md) for full documentation.

**Script**: [`pr-swarmit/pr-swarmit.sh`](./pr-swarmit/pr-swarmit.sh)

**Quick Start**:
```
/pr-swarmit                    # Analyze all open PRs
/pr-swarmit --prs 56,54,53     # Fix specific PRs
/pr-swarmit --dry-run          # Preview without changes
```

**What it does**:
1. Lists all open PRs with CI status
2. Categorizes failures (coverage, compilation, formatting, etc.)
3. Launches parallel sub-agents to fix issues
4. Tracks progress and results

**Common use cases**:
- Fix coverage ratchet failures across multiple PRs
- Resolve Rust compilation errors blocking CI
- Apply formatting fixes in parallel
- Prepare multiple PRs for merging simultaneously

## Creating New Skills

To add a new skill:

1. Create a skill directory: `mkdir -p .claude/skills/<skill-name>/`
2. Create `SKILL.md` with YAML frontmatter
3. Add optional supporting files (scripts, templates, examples)
4. Update this README

**Skill directory structure**:
```
<skill-name>/
├── SKILL.md           # Main instructions with YAML frontmatter (required)
├── script.sh          # Optional executable script
├── template.md        # Optional template for Claude to fill in
└── examples/          # Optional example outputs
    └── sample.md
```

**SKILL.md frontmatter template**:
```markdown
---
name: skill-name
description: What this skill does and when to use it
argument-hint: [optional-args]
disable-model-invocation: true
allowed-tools: Bash, Read, Grep
---

# Skill Name

Detailed instructions...
```

**Skill naming convention**:
- Use kebab-case for directory names
- Keep names short and memorable
- Include usage examples
- Use `disable-model-invocation: true` for skills with side effects

## See Also

- [CLAUDE.md](../../CLAUDE.md) - Project overview
- [Claude Code Skills Documentation](https://code.claude.com/docs/en/skills)
- [../settings.json](../settings.json) - Claude Code configuration
