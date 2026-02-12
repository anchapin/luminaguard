#!/bin/bash

# List of PRs we fixed that should be ready to merge
PRS="87 86 85 84 83 80 79 76 75 74 71 70 68 67 66"

echo "Checking CI status for fixed PRs..."
for pr in $PRS; do
  echo "Checking PR #$pr..."
  status=$(gh pr view $pr --json statusCheckRollup --jq '.statusCheckRollup[] | select(.conclusion == "FAILURE") | length')
  if [ "$status" -eq 0 ]; then
    echo "  ✅ PR #$pr - All checks passing"
  else
    echo "  ⚠️  PR #$pr - Has failing checks"
  fi
done

echo ""
echo "Ready to merge PRs with passing CI checks:"
for pr in $PRS; do
  status=$(gh pr view $pr --json statusCheckRollup --jq '.statusCheckRollup[] | select(.conclusion == "FAILURE") | length')
  if [ "$status" -eq 0 ]; then
    echo "#$pr"
  fi
done
