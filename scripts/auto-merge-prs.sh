#!/bin/bash
# Auto-merge Script for IronClaw PRs
# Dynamically discovers open PRs that are ready to merge and merges them
# Updates GitHub issue #93 with progress

set -e  # Exit on error

# Configuration
TRACKING_ISSUE=93
POLL_INTERVAL=60  # Check every 60 seconds
MAX_WAIT=1800     # Wait max 30 minutes (1800 seconds)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "═══════════════════════════════════════════════════"
echo "           IronClaw Auto-Merge Script"
echo "           (Dynamic PR Discovery)"
echo "═══════════════════════════════════════════════════════"
echo ""

# Function to check CI status from statusCheckRollup JSON
check_ci_from_json() {
  local rollup=$1

  # Parse the statusCheckRollup and return the overall status
  local status=$(echo "$rollup" | jq -r '
    if . == null or length == 0 then
      "UNKNOWN"
    else
      [.[] |
        if .conclusion == "FAILURE" then
          "FAIL"
        elif .conclusion == "SUCCESS" or .conclusion == "SKIPPED" or .conclusion == "NEUTRAL" then
          "SUCCESS"
        elif .status == "IN_PROGRESS" or .status == "PENDING" or .status == "QUEUED" then
          "RUNNING"
        elif .conclusion == null then
          "RUNNING"
        else
          "UNKNOWN"
        end
      ] | if any(. == "FAIL") then
           "FAIL"
         elif any(. == "RUNNING") then
           "RUNNING"
         elif all(. == "SUCCESS") then
           "SUCCESS"
         else
           "UNKNOWN"
         end
    end')

  echo "$status"
}

# Function to discover ready-to-merge PRs
# Returns space-separated list of PR numbers sorted by creation date (oldest first)
discover_ready_prs() {
  local ready_prs=""

  # Get all open PRs with their status
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue

    local pr_number=$(echo "$line" | cut -d'|' -f1)
    local created_at=$(echo "$line" | cut -d'|' -f2)
    local is_draft=$(echo "$line" | cut -d'|' -f3)
    local mergeable=$(echo "$line" | cut -d'|' -f4)
    local status_rollup=$(echo "$line" | cut -d'|' -f5-)

    # Skip drafts
    [[ "$is_draft" == "true" ]] && continue

    # Skip not mergeable (has conflicts)
    # Note: mergeable can be "MERGEABLE" or "true" (boolean in JSON becomes string)
    [[ "$mergeable" != "true" && "$mergeable" != "MERGEABLE" ]] && continue

    # Check CI status
    local ci_status=$(check_ci_from_json "$status_rollup")

    # Only include PRs with all checks passing
    if [[ "$ci_status" == "SUCCESS" ]]; then
      ready_prs="$ready_prs$pr_number|$created_at\n"
    fi

  done < <(gh pr list \
    --state open \
    --limit 100 \
    --json number,createdAt,isDraft,mergeable,statusCheckRollup \
    --jq '.[] | "\(.number)|\(.createdAt)|\(.isDraft)|\(.mergeable)|\(.statusCheckRollup)"')

  # Sort by creation date and extract just PR numbers
  if [[ -n "$ready_prs" ]]; then
    echo -e "$ready_prs" | sort -t'|' -k2 | cut -d'|' -f1 | tr '\n' ' ' | xargs
  else
    echo ""
  fi
}

# Function to get PR title for commit subject
get_pr_title() {
  local pr=$1
  gh pr view "$pr" --json title --jq '.title'
}

# Function to check CI status for a PR (for polling)
check_pr_status() {
  local pr=$1

  local status=$(gh pr view "$pr" --json statusCheckRollup | jq -r '
    [.statusCheckRollup[] |
      if .conclusion == "FAILURE" then
        "FAIL"
      elif .conclusion == "SUCCESS" or .conclusion == "SKIPPED" or .conclusion == "NEUTRAL" then
        "SUCCESS"
      elif .status == "IN_PROGRESS" or .status == "PENDING" or .status == "QUEUED" then
        "RUNNING"
      elif .conclusion == null then
        "RUNNING"
      else
        "UNKNOWN"
      end
    ] | if any(. == "FAIL") then
         "FAIL"
       elif any(. == "RUNNING") then
         "RUNNING"
       elif any(. == "SUCCESS") then
         "SUCCESS"
       else
         "UNKNOWN"
       end')

  echo "$status"
}

# Function to merge a single PR
merge_pr() {
  local pr=$1
  local subject=$2

  echo -e "${GREEN}Merging PR #$pr...${NC}"

  # Capture output to check for errors
  local output
  if output=$(gh pr merge "$pr" --squash --delete-branch --admin --subject "$subject" 2>&1); then
    echo -e "${GREEN}✅ PR #$pr merged successfully!${NC}"

    # Update tracking issue
    local comment_body="✅ Merged PR #$pr: $subject"
    gh issue comment "$TRACKING_ISSUE" --body "$comment_body"

    return 0
  else
    local error_msg="$output"

    # Check for merge conflicts
    if echo "$error_msg" | grep -q "not mergeable"; then
        echo -e "${YELLOW}⚠️  Merge conflict detected for PR #$pr. Attempting to resolve...${NC}"

        # Check for existing worktrees for this PR branch
        # We need to know the branch name first. gh pr view can tell us.
        local head_ref=$(gh pr view "$pr" --json headRefName --jq '.headRefName')

        # Check if this branch is checked out in any worktree
        local worktree_path=$(git worktree list | grep "$head_ref" | awk '{print $1}')

        if [[ -n "$worktree_path" ]]; then
            echo -e "${YELLOW}⚠️  Worktree detected for branch $head_ref at $worktree_path. Removing it...${NC}"
            if git worktree remove --force "$worktree_path"; then
                echo -e "${GREEN}✅ Worktree removed.${NC}"
            else
                echo -e "${RED}❌ Failed to remove worktree at $worktree_path. Cannot proceed safely.${NC}"
                return 1
            fi
        fi

        # Checkout PR
        echo "Checking out PR #$pr..."
        if ! gh pr checkout "$pr"; then
            echo -e "${RED}❌ Failed to checkout PR #$pr. Aborting conflict resolution to protect current branch.${NC}"

            local failure_body=$'❌ Failed to resolve conflicts for PR #'"$pr"$' locally: Could not checkout branch.\n\nManual intervention required.'
            gh issue comment "$TRACKING_ISSUE" --body "$failure_body"

            return 1
        fi

        # Configure git if needed
        git config user.name "Auto Merge Bot"
        git config user.email "auto-merge@ironclaw.app"

        # Merge main with "ours" strategy (keeping PR changes)
        echo "Merging origin/main with strategy 'ours'..."
        git fetch origin main
        if git merge origin/main -X ours -m "Merge main (resolving conflicts)"; then
            echo "Pushing resolution..."
            git push
            echo -e "${GREEN}✅ Conflicts resolved and pushed for PR #$pr. Waiting for checks...${NC}"

             local comment_body=$'🔄 Resolved merge conflicts for PR #'"$pr"$' (kept PR changes). Waiting for checks.'
             gh issue comment "$TRACKING_ISSUE" --body "$comment_body"

             return 1 # Return failure so it stays in loop and waits for new checks
        else
            echo -e "${RED}❌ Failed to resolve conflicts for PR #$pr locally${NC}"
            # Attempt to reset to avoid leaving mess
            git merge --abort 2>/dev/null || true
        fi
    fi

    echo -e "${RED}❌ Failed to merge PR #$pr${NC}"
    echo -e "${YELLOW}Error: $error_msg${NC}"

    # Report failure to tracking issue
    local failure_body=$'❌ Failed to merge PR #'"$pr"$':\n\n```\n'"$error_msg"$'\n```\n\nManual intervention required.'
    gh issue comment "$TRACKING_ISSUE" --body "$failure_body"

    return 1
  fi
}

# Main execution
main() {
  local elapsed=0

  # Discover ready PRs dynamically
  echo -e "${YELLOW}Discovering ready-to-merge PRs...${NC}"
  local ready_prs=$(discover_ready_prs)

  if [[ -z "$ready_prs" ]]; then
    echo -e "${YELLOW}No ready PRs found (open, mergeable, CI passing).${NC}"
    echo "Exiting gracefully."
    return 0
  fi

  local pending_prs="$ready_prs"
  local pr_count=$(echo $pending_prs | wc -w)

  echo -e "${GREEN}Found $pr_count ready PR(s): $pending_prs${NC}"
  echo ""

  # Update tracking issue
  local start_msg=$'🚀 **AUTO-MERGE STARTED**\n\nDynamically discovered '"$pr_count"' ready PR(s): '"$pending_prs"$'\n\nMerging from oldest to newest.'
  gh issue comment "$TRACKING_ISSUE" --body "$start_msg"

  # Polling loop
  while [ $elapsed -lt $MAX_WAIT ]; do
    local new_pending_prs=""
    local merged_any=0

    # Check status of each pending PR
    for pr in $pending_prs; do
      local status=$(check_pr_status "$pr")

      if [[ "$status" == "SUCCESS" ]]; then
        # Merge the PR
        local subject=$(get_pr_title "$pr")
        if merge_pr "$pr" "$subject"; then
          merged_any=1
          continue # Skip adding to new_pending_prs
        fi
      fi

      # If not merged (status not success, or merge failed), keep in pending
      new_pending_prs="$new_pending_prs $pr"
    done

    # Update pending list (trim leading space)
    pending_prs=$(echo $new_pending_prs | xargs)

    # Check if all PRs are processed
    if [[ -z "$pending_prs" ]]; then
      echo ""
      echo -e "${GREEN}═══════════════════════════════${NC}"
      echo -e "${GREEN}All PRs merged successfully!${NC}"
      echo -e "${GREEN}═══════════════════════════════${NC}"

      local complete_msg=$'✅ **AUTO-MERGE COMPLETE**\n\nAll PRs have been processed.'
      gh issue comment "$TRACKING_ISSUE" --body "$complete_msg"

      return 0
    fi

    # Show current status headers
    echo -ne "\r                                                                                \r"
    echo -ne " PR Status                                                                                 "
    echo -ne "─────                                                                                         "

    # Display status for remaining PRs
    for pr in $pending_prs; do
        local status=$(check_pr_status "$pr")
        local symbol=""
        local color=""

        case $status in
          SUCCESS)
            symbol="✅"
            color="$GREEN"
            ;;
          RUNNING)
            symbol="⏳"
            color="$YELLOW"
            ;;
          FAIL)
            symbol="❌"
            color="$RED"
            ;;
          *)
            symbol="?"
            color="$NC"
            ;;
        esac

        printf " ${color}#$pr ${symbol}${NC} "
    done

    echo ""
    echo -e "${YELLOW}Elapsed: $((elapsed / 60))m:$((elapsed % 60))s | Waiting: $(((MAX_WAIT - elapsed) / 60))m:$(((MAX_WAIT - elapsed) % 60))s | Pending: $(echo $pending_prs | wc -w)${NC}"

    # Wait before next poll
    sleep $POLL_INTERVAL
    elapsed=$((elapsed + POLL_INTERVAL))
  done

  # Timeout reached
  echo ""
  echo -e "${RED}═══════════════════════════════════${NC}"
  echo -e "${RED}TIMEOUT: Auto-merge timed out after $((MAX_WAIT / 60)) minutes${NC}"
  echo -e "${RED}═══════════════════════════════════${NC}"
  echo ""
  echo "Remaining Pending PRs: $pending_prs"

  # Report timeout to tracking issue
  local timeout_msg=$'⚠️ **AUTO-MERGE TIMEOUT**\n\nAuto-merge script timed out.\n\nPending PRs: '"$pending_prs"$'\n\nManual intervention required.\n\nElapsed time: '"$((elapsed / 60))"$' minutes'
  gh issue comment "$TRACKING_ISSUE" --body "$timeout_msg"

  return 1
}

# Run main function
main "$@"
