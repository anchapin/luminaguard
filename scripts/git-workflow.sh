#!/bin/bash
# IronClaw Git Workflow Helper
# Automates feature branch workflow with issue tracking

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}ℹ${NC} $1"; }
warn() { echo -e "${YELLOW}⚠${NC} $1"; }
error() { echo -e "${RED}❌${NC} $1"; }
success() { echo -e "${GREEN}✅${NC} $1"; }

# Show help
show_help() {
  cat << EOF
${BLUE}IronClaw Git Workflow Helper${NC}

${GREEN}Usage:${NC}
  $0 <command> [arguments]

${GREEN}Commands:${NC}
  ${BLUE}start${NC} ISSUE-NUM [description]   Start new feature branch from issue
  ${BLUE}submit${NC}                          Create pull request for current branch
  ${BLUE}status${NC}                          Show current branch and linked issue
  ${BLUE}sync${NC}                            Sync feature branch with main
  ${BLUE}help${NC}                            Show this help message

${GREEN}Examples:${NC}
  $0 start 42 "Add MCP client connection"
  $0 submit
  $0 status

${GREEN}Philosophy:${NC}
  • All work must link to a GitHub issue
  • All changes must go through PRs
  • Direct commits to main are blocked
  • AI agents follow same workflow as humans

${GREEN}Workflow:${NC}
  1. Create issue: ${BLUE}gh issue create --title 'Description'${NC}
  2. Start branch: ${BLUE}$0 start ISSUE-NUM 'description'${NC}
  3. Make changes and commit
  4. Submit PR: ${BLUE}$0 submit${NC}

Documentation: See CLAUDE.md section "Git Workflow (AI-Agent Enforced)"
EOF
}

# Command: start - Begin new feature
cmd_start() {
  local ISSUE=$1
  local DESC=${2:-"feature"}

  if [ -z "$ISSUE" ]; then
    error "Issue number required"
    echo ""
    echo "Usage: $0 start ISSUE-NUM [description]"
    echo ""
    echo "Example: $0 start 42 'Add MCP client'"
    exit 1
  fi

  # Validate issue exists
  info "Checking issue #$ISSUE..."
  if ! gh issue view "$ISSUE" &>/dev/null; then
    error "Issue #$ISSUE does not exist"
    echo ""
    echo "Create it first:"
    echo "  gh issue create --title '$DESC' --body 'Implementation details...'"
    exit 1
  fi

  # Get issue details
  local TITLE=$(gh issue view "$ISSUE" --json title -q .title)
  local STATE=$(gh issue view "$ISSUE" --json state -q .state)

  info "Issue #$ISSUE found"
  echo "  Title: $TITLE"
  echo "  State: $STATE"

  if [ "$STATE" != "OPEN" ]; then
    warn "Issue is not open (state: $STATE)"
  fi

  # Create branch
  local BRANCH="feature/$ISSUE-${DESC// /-}"
  echo ""

  # Check if branch already exists
  if git show-ref --verify --quiet "refs/heads/$BRANCH"; then
    warn "Branch $BRANCH already exists"
    read -p "Switch to existing branch? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
      git checkout "$BRANCH"
      success "Switched to $BRANCH"
    else
      exit 0
    fi
  else
    # Ensure we're on main first
    local CURRENT=$(git rev-parse --abbrev-ref HEAD)
    if [ "$CURRENT" != "main" ]; then
      info "Switching to main branch first..."
      git checkout main
    fi

    git checkout -b "$BRANCH"
    success "Created branch: $BRANCH"
  fi

  echo ""
  info "Next steps:"
  echo "  1. Make your changes"
  echo "  2. Commit: git commit -m 'Description'"
  echo "  3. Submit PR: $0 submit"
}

# Command: submit - Create PR
cmd_submit() {
  local CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

  # Validate we're not on main
  if [ "$CURRENT_BRANCH" = "main" ]; then
    error "Cannot create PR from main branch"
    exit 1
  fi

  # Validate branch name format
  if [[ ! "$CURRENT_BRANCH" =~ ^feature/[0-9]+- ]]; then
    warn "Branch name doesn't match feature/ISSUE-NUM-description"
    echo "  Current: $CURRENT_BRANCH"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
      exit 1
    fi
    local ISSUE=""
  else
    # Extract issue number from branch name
    local ISSUE=$(echo "$CURRENT_BRANCH" | cut -d- -f2)
    info "Extracted issue #$ISSUE from branch name"
  fi

  # Check if PR already exists
  info "Checking for existing PRs..."
  if gh pr list --head "$CURRENT_BRANCH" --json number | jq -e '.[0].number' > /dev/null 2>&1; then
    local PR_NUM=$(gh pr list --head "$CURRENT_BRANCH" --json number -q '.[0].number')
    warn "PR already exists for this branch"
    echo "  PR #$PR_NUM"
    gh pr view "$PR_NUM" --web
    exit 0
  fi

  # Build PR title and body
  if [ -n "$ISSUE" ]; then
    local ISSUE_TITLE=$(gh issue view "$ISSUE" --json title -q .title)
    local PR_TITLE="Work on #$ISSUE: $ISSUE_TITLE"
    local PR_BODY="Closes #$ISSUE"
  else
    local PR_TITLE="Work on $CURRENT_BRANCH"
    local PR_BODY="Work on $(git rev-parse --abbrev-ref HEAD)"
  fi

  # Create PR
  echo ""
  info "Creating pull request..."
  gh pr create \
    --title "$PR_TITLE" \
    --body "$PR_BODY" \
    --base main \
    --label "needs-review" 2>&1 || {
    error "Failed to create PR"
    echo ""
    echo "Possible reasons:"
    echo "  • No commits to push"
    echo "  • Branch not pushed to remote"
    echo "  • PR already exists"
    exit 1
  }

  success "Pull request created!"

  # Open in browser
  gh pr view --web
}

# Command: status - Show workflow status
cmd_status() {
  local CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

  echo "Current branch: $CURRENT_BRANCH"
  echo ""

  if [[ "$CURRENT_BRANCH" =~ ^feature/[0-9]+- ]]; then
    local ISSUE=$(echo "$CURRENT_BRANCH" | cut -d- -f2)

    info "Linked issue #$ISSUE"
    gh issue view "$ISSUE" --json title,state,url,labels | jq -r '
      "  Title: \(.title)\n  State: \(.state)\n  Labels: \([.labels[].name] | join(", ") | if . == "" then "none" else . end)\n  URL: \(.url)"
    '

    echo ""
    info "Pull Requests:"
    if gh pr list --head "$CURRENT_BRANCH" --json number,title,state,reviewDecision | jq -e '.[0]' > /dev/null 2>&1; then
      gh pr list --head "$CURRENT_BRANCH" --json number,title,state,reviewDecision | jq -r '
        if .[0].reviewDecision == "APPROVED" then
          "  PR #\(.[0].number): \(.[0].title) ✅ Approved"
        elif .[0].reviewDecision == "REVIEW_REQUIRED" then
          "  PR #\(.[0].number): \(.[0].title) 👀 Review required"
        else
          "  PR #\(.[0].number): \(.[0].title) (\(.[0].state))"
        end
      '
    else
      echo "  No PR created yet. Run: $0 submit"
    fi
  else
    warn "Not a feature branch (no issue linkage)"
    echo ""
    echo "Start a new feature:"
    echo "  $0 start ISSUE-NUM 'description'"
  fi
}

# Command: sync - Sync with main
cmd_sync() {
  local CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

  if [ "$CURRENT_BRANCH" = "main" ]; then
    error "Already on main branch"
    exit 1
  fi

  info "Fetching latest from origin..."
  git fetch origin main

  info "Rebasing $CURRENT_BRANCH onto main..."
  git rebase origin/main

  success "Sync complete"
}

# Main
case "${1:-}" in
  start)
    cmd_start "$2" "$3"
    ;;
  submit)
    cmd_submit
    ;;
  status)
    cmd_status
    ;;
  sync)
    cmd_sync
    ;;
  help|--help|-h)
    show_help
    ;;
  *)
    if [ -z "$1" ]; then
      show_help
    else
      error "Unknown command: $1"
      echo ""
      show_help
      exit 1
    fi
    ;;
esac
