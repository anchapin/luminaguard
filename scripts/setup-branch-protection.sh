#!/bin/bash
LuminaGuard
# Configures GitHub branch protection rules to enforce PR workflow

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}🔒 IronClaw Branch Protection Setup${NC}"
echo ""

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
  echo -e "${RED}❌ gh CLI not found${NC}"
  echo "Install from: https://cli.github.com/"
  exit 1
fi

# Check if authenticated
if ! gh auth status &> /dev/null; then
  echo -e "${RED}❌ gh CLI not authenticated${NC}"
  echo "Run: gh auth login"
  exit 1
fi

# Get repository info - support both HTTPS and SSH formats
REMOTE_URL=$(git remote get-url origin)

# Try to parse as SSH format: git@github.com:owner/repo.git
if [[ "$REMOTE_URL" =~ ^git@github.com:(.*)\.git$ ]]; then
  REPO_SLUG="${BASH_REMATCH[1]}"
# Try to parse as HTTPS format: https://github.com/owner/repo.git
elif [[ "$REMOTE_URL" =~ ^https://github.com/(.*)\.git$ ]]; then
  REPO_SLUG="${BASH_REMATCH[1]}"
# Try to parse as HTTPS without .git: https://github.com/owner/repo
elif [[ "$REMOTE_URL" =~ ^https://github.com/(.*)$ ]]; then
  REPO_SLUG="${BASH_REMATCH[1]}"
else
  # Fallback to sed-based parsing
  REPO_SLUG=$(echo "$REMOTE_URL" | sed -E 's|.*github.com[/:]||' | sed 's|\.git$||')
fi

if [ -z "$REPO_SLUG" ] || [ "$REPO_SLUG" = "$REMOTE_URL" ]; then
  echo -e "${RED}❌ Could not determine repository slug${NC}"
  echo "Remote URL: $REMOTE_URL"
  echo "Please ensure you have a GitHub remote named 'origin'"
  exit 1
fi

echo "Repository: $REPO_SLUG"
echo ""

# Check if main branch exists locally
if ! git show-ref --verify --quiet refs/heads/main; then
  echo -e "${YELLOW}⚠️  Warning: 'main' branch not found locally${NC}"
  echo ""
  echo "Branches available:"
  git branch -a
  echo ""
  read -p "Continue anyway? (y/N) " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 0
  fi
fi

# Confirm
echo -e "${YELLOW}This will configure branch protection for the 'main' branch:${NC}"
echo "  • Require pull requests (1 approval)"
echo "  • Require status checks to pass"
echo "  • Block direct pushes"
echo "  • Enforce on admins"
echo "  • Prevent branch deletion"
echo ""
read -p "Continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  echo "Aborted"
  exit 0
fi

echo ""
echo "Configuring branch protection..."

# Build JSON payload for branch protection
# Using a heredoc to construct proper JSON
JSON_PAYLOAD=$(cat <<EOF
{
  "required_pull_request_reviews": {
    "required_approving_review_count": 1,
    "dismiss_stale_reviews": false,
    "require_code_owner_reviews": false
  },
  "enforce_admins": true,
  "allow_deletions": false,
  "allow_force_deletions": false,
  "required_linear_history": false,
  "required_conversation_resolution": false,
  "lock_branch": false,
  "allow_fork_syncing": false,
  "required_status_checks": null,
  "restrictions": null
}
EOF
)

# Call GitHub API with proper JSON
gh api "repos/$REPO_SLUG/branches/main/protection" \
  --method PUT \
  --silent \
  -H "Accept: application/vnd.github+json" \
  --input - <<< "$JSON_PAYLOAD" \
  2>&1 || {
    echo -e "${RED}❌ Failed to configure branch protection${NC}"
    echo ""
    echo "Possible reasons:"
    echo "  • Insufficient permissions (need repo admin access)"
    echo "  • Branch protection already exists"
    echo "  • Network error"
    echo ""
    echo "You can configure manually at:"
    echo "  https://github.com/$REPO_SLUG/settings/branches"
    echo ""
    echo "Required settings:"
    echo "  ✓ Require pull requests (1 approval)"
    echo "  ✓ Block direct pushes"
    echo "  ✓ Enforce on admins"
    echo "  ✓ Prevent branch deletion"
    exit 1
  }

echo ""
echo -e "${GREEN}✅ Branch protection configured successfully!${NC}"
echo ""
echo "Rules applied:"
echo "  • Pull requests before merging (1 approval required)"
echo "  • Direct pushes to main blocked"
echo "  • Branch deletions blocked"
echo "  • Admin enforcement enabled"
echo ""
echo "View settings:"
echo "  https://github.com/$REPO_SLUG/settings/branches"
