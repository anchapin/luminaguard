# Auto-Merge Script for IronClaw PRs

## Overview

This script automatically monitors CI status for 14 IronClaw PRs and merges them when all checks pass.

## Features

- ✅ **Real-time monitoring** - Polls CI status every 60 seconds
- 🎯 **Automatic merging** - Merges all PRs when CI passes
- 📊 **Progress tracking** - Updates GitHub issue #93 with status
- 🚨 **Error handling** - Reports merge failures and requires manual intervention
- ⏱️ **Timeout protection** - 30-minute maximum wait time

## PRs Included

The script will merge the following PRs in order:

1. **#86** - CRITICAL: Fix critical vulnerabilities (collision, path traversal, seccomp)
2. **#87** - Review feedback (tests, coverage, clippy)
3. **#83, #84, #85** - Formatting fixes
4. **#80** - Windows guards + coverage
5. **#79** - Formatting + coverage (76.9%!)
6. **#76, #75** - Formatting fixes
7. **#74** - Method name + module structure
8. **#71, #70, #68, #67, #66** - Windows platform fixes

## Usage

### Basic Usage

```bash
# Run the script (interactive)
./scripts/auto-merge-prs.sh

# Or run in background with nohup
nohup ./scripts/auto-merge-prs.sh > /tmp/merge.log 2>&1 &

# Monitor progress
tail -f /tmp/merge.log
```

### Configuration

You can modify these variables at the top of the script:

- `TRACKING_ISSUE=93` - GitHub issue number for progress updates
- `PRS_TO_MERGE` - Space-separated list of PR numbers
- `POLL_INTERVAL=60` - Seconds between status checks
- `MAX_WAIT=1800` - Maximum wait time in seconds (30 minutes)

## What It Does

1. **Polling Phase**
   - Checks CI status for all PRs every 60 seconds
   - Displays color-coded status (✅ PASSING, ⏳ RUNNING, ❌ FAILED)
   - Shows elapsed time and remaining wait time

2. **Auto-Merge Phase** (when all PRs pass)
   - Adds comment to GitHub issue #93 announcing start
   - Merges each PR with appropriate commit message
   - Deletes branch after successful merge
   - Updates issue #93 after each merge
   - Adds final completion comment when done

3. **Error Handling**
   - If merge fails, reports error to issue #93
   - Stops execution and requires manual intervention
   - Preserves error messages for debugging

4. **Timeout Protection**
   - If CI doesn't complete within 30 minutes
   - Reports timeout to issue #93
   - Displays final status of all PRs
   - Exits with error code

## Output

The script provides real-time progress updates:

```
══════════════════════════════════════════════════════
           IronClaw Auto-Merge Script
════════════════════════════════════════════════════════════

Waiting for CI checks to complete...
Polling every 60s...

 PR Status                                                                                  
─────                                                                                        
#86 ⏳ Fix Critical Collision and Path Traversal Vulnerabilities                        
#87 ⏳ Review Feedback for PR 77                                               
#83 ✅ PR Review #69 Findings                                                       
...

Elapsed: 2m:45s | Waiting: 27m:15s
```

## Tracking

All progress is automatically posted to GitHub issue #93:
- Start announcement
- Per-PR merge confirmations
- Final completion summary
- Any errors or timeouts

## Manual Intervention Required

The script will stop and require manual intervention if:

- ❌ Any PR check fails
- 🚨 Merge operation fails (conflicts, permissions, etc.)
- ⏱️ Timeout (30 minutes) reached

In these cases, check issue #93 for details and run suggested commands.

## Troubleshooting

### Script won't run
```bash
chmod +x scripts/auto-merge-prs.sh
./scripts/auto-merge-prs.sh
```

### CI checks not passing
The script will wait indefinitely (up to MAX_WAIT). Check status manually:
```bash
gh pr checks <PR-number>
```

### Merge conflicts
If merge fails due to conflicts:
```bash
# Resolve conflicts
gh pr checkout <PR-number>
git merge origin/main
# Fix conflicts
git push
```

Then re-run the script or merge manually.

## Safety Features

- ✅ Read-only operation (only merges, doesn't modify code)
- ✅ Uses `--squash` for clean commit history
- ✅ `--delete-branch` cleans up after merge
- ✅ All actions tracked in GitHub issue #93
- ✅ Non-zero exit on any error
- ✅ Timeout protection prevents hanging

## See Also

- [Merge Order Plan](../docs/PR_MERGE_PLAN.md) - Detailed merge strategy
- [Issue #93](https://github.com/anchapin/ironclaw/issues/93) - Live progress tracker
