# Jules GitHub Integration Setup Guide

This guide walks through configuring Jules AI agent to work with the LuminaGuard repository for PR reviews and auto-fixing CI failures.

## Overview

Created workflows:
- **jules-pr-review.yml** - Automatically reviews PRs and provides feedback
- **jules-ci-fix.yml** - Auto-fixes failing CI checks
- **jules-bug-fixer.yml** - Diagnoses and fixes bugs labeled with "bug" label

---

## Step 1: Configure Jules Web App

1. Go to https://jules.google/
2. Click on your repository (under "codebases")
3. Select **"Configuration"** at the top
4. In **"Initial Setup"**, enter:
   ```bash
   make install
   ```
5. Click **"Run and Snapshot"**

This ensures Jules understands how to set up the LuminaGuard development environment.

---

## Step 2: Add Jules API Key to GitHub Secrets

1. Go to your repository: https://github.com/anchapin/ironclaw
2. Navigate to **Settings** → **Secrets and variables** → **Actions**
3. Click **"New repository secret"**
4. Name: `JULES_API_KEY`
5. Value: Your API key (from https://jules.google/ → Settings)
6. Click **"Add secret"**

**OR** via GitHub CLI:
```bash
gh secret set JULES_API_KEY --body "AQ.Ab8RN6LYMJNu2fXHSVQeNMm2v7VvbSfv8nwXcEcRw2KiJaOHNQ"
```

---

## Step 3: Workflow Files

The following workflow files have been created in `.github/workflows/`:

### jules-pr-review.yml
**Trigger:** Pull requests (opened, updated, reopened)

**What it does:**
- Reviews PRs for code quality, security, and adherence to project standards
- Checks for potential bugs and issues
- Provides constructive feedback as PR comments
- Verifies adherence to LuminaGuard constraints (loop.py line limit, coverage, etc.)

**No manual action needed** - runs automatically on PRs.

### jules-ci-fix.yml
**Trigger:** When "LuminaGuard CI" or "quality-gates" workflows fail

**What it does:**
- Analyzes the CI failure
- Identifies root cause
- Implements a fix
- Creates a PR with the fix on the failing branch

**No manual action needed** - runs automatically when CI fails.

### jules-bug-fixer.yml
**Trigger:** Issues labeled with `bug` label

**What it does:**
- Diagnoses the bug from the issue description
- Traces through the codebase
- Implements a fix with regression tests
- Creates a PR following TDD workflow

**Usage:**
1. Create a GitHub issue describing the bug
2. Add the `bug` label to the issue
3. Jules will automatically start working on it

**Security note:** This workflow only triggers for trusted users (`alexc`, `anchapin`).

---

## Step 4: Trusted Users Configuration

For the bug-fixer workflow, you should add all trusted collaborators who can trigger Jules:

Edit `.github/workflows/jules-bug-fixer.yml`:

```yaml
# SECURITY: Only allow trusted users to trigger Jules
# Add your GitHub username and trusted collaborators here
if: ${{ contains(fromJSON('["alexc", "anchapin", "other-user"]'), github.event.issue.user.login) }}
```

---

## Step 5: Verify Setup

### Check workflow files are in place:
```bash
ls -la .github/workflows/jules-*.yml
```

### Test the configuration:

**Test PR Review:**
1. Create a test branch: `git checkout -b test/jules-review`
2. Make a small change and commit
3. Push and create a PR
4. Jules should automatically add a review comment

**Test CI Fix:**
1. Intentionally break a test
2. Push to a branch
3. Wait for CI to fail
4. Jules should automatically attempt to fix it

**Test Bug Fixer:**
1. Create a new issue with a bug report
2. Add the `bug` label
3. Jules should start working on a fix

---

## Step 6: Monitor Jules Activity

### View Jules Sessions:
- Go to https://jules.google/
- Click on your repository
- View active and past sessions

### Review Jules PRs:
- Jules creates PRs with descriptive titles
- Always review the code before merging
- Check that tests pass
- Verify the fix addresses the issue

---

## Important Notes

### Permissions
The workflows use minimal permissions:
```yaml
permissions:
  contents: read
  pull-requests: write  # Only for PR review workflow
  actions: read  # Only for CI fix workflow
  issues: read  # Only for bug fixer workflow
```

### Security Best Practices
1. **Never commit** `JULES_API_KEY` - always use GitHub Secrets
2. **Review Jules PRs** before merging - Jules is helpful but not infallible
3. **Trusted users only** - Bug fixer only works for allowlisted users
4. **Monitor activity** - Check Jules sessions regularly

### Costs and Quotas
- Jules API has usage limits (check your account at jules.google)
- Each workflow run consumes API quota
- CI auto-fix can trigger multiple times on the same failure

### Troubleshooting

**Jules doesn't trigger:**
- Check that `JULES_API_KEY` secret is set correctly
- Verify workflow file syntax (GitHub will show errors)
- Check Actions tab for workflow run logs

**Jules creates poor fixes:**
- Review the prompt in the workflow file
- Add more context about your codebase
- Report issues to https://github.com/google-labs-code/jules-action

**Too many Jules PRs:**
- Add conditions to limit triggers (e.g., only on specific branches)
- Use the `workflow_dispatch` trigger for manual control
- Disable problematic workflows temporarily

---

## Advanced Configuration

### Manual Trigger
Add to any workflow to run manually from GitHub UI:
```yaml
on:
  workflow_dispatch:
    inputs:
      task:
        description: 'Task for Jules'
        required: true
        type: string
```

### Scheduled Tasks
Run Jules on a schedule (e.g., daily security scans):
```yaml
on:
  schedule:
    - cron: '0 6 * * *'  # Daily at 6 AM UTC
```

### Conditional Execution
```yaml
# Only run on specific branches
if: github.event.pull_request.base.ref == 'main'

# Only run if files changed
if: contains(github.event.pull_request.changed_files, 'loop.py')

# Only run for specific users
if: github.actor == 'alexc'
```

---

## Resources

- **Jules Web App:** https://jules.google/
- **Jules API Docs:** https://developers.google.com/jules/api
- **GitHub Action Repo:** https://github.com/google-labs-code/jules-action
- **Support:** https://support.google.com/gemini/

---

## Feedback and Iteration

After running these workflows for a while, you may want to:

1. **Adjust prompts** based on Jules' performance
2. **Add more constraints** specific to your workflow
3. **Refine the trusted users list**
4. **Add more workflows** (e.g., daily security scans, performance improvements)

To update a workflow:
1. Edit the YAML file in `.github/workflows/`
2. Commit and push
3. GitHub Actions will use the updated workflow on next trigger

---

## Example: Creating a Custom Workflow

Want Jules to do something specific? Create a new workflow:

```yaml
name: Jules - Custom Task

on:
  workflow_dispatch:  # Manual trigger

jobs:
  custom-task:
    runs-on: ubuntu-latest
    steps:
      - uses: google-labs-code/jules-invoke@v1
        with:
          prompt: |
            Your custom prompt here...
            Be specific about what you want Jules to do.
          jules_api_key: ${{ secrets.JULES_API_KEY }}
```

Save as `.github/workflows/jules-custom.yml` and trigger from the Actions tab.
