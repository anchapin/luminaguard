#!/bin/bash

# Quick start script for auto-merge
# Runs the auto-merge script in background with logging

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="/tmp/ironclaw-auto-merge.log"
PID_FILE="/tmp/ironclaw-auto-merge.pid"

echo "════════════════════════════════════════════════════════════════════"
echo "           IronClaw Auto-Merge Launcher"
echo "════════════════════════════════════════════════════════════════════"
echo ""

# Check if already running
if [ -f "$PID_FILE" ]; then
  old_pid=$(cat "$PID_FILE")
  if ps -p "$old_pid" > /dev/null 2>&1; then
    echo "⚠️  Auto-merge is already running (PID: $old_pid)"
    echo ""
    echo "To monitor progress:"
    echo "  tail -f $LOG_FILE"
    echo ""
    echo "To stop it:"
    echo "  kill $old_pid"
    exit 1
  else
    # Stale PID file, remove it
    rm -f "$PID_FILE"
  fi
fi

echo "Starting auto-merge in background..."
echo "Log file: $LOG_FILE"
echo "PID file: $PID_FILE"
echo ""

# Run the auto-merge script in background
chmod +x "$SCRIPT_DIR/auto-merge-prs.sh"
nohup "$SCRIPT_DIR/auto-merge-prs.sh" > "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"

echo "✅ Auto-merge started!"
echo ""
echo "Monitor progress with:"
echo "  tail -f $LOG_FILE"
echo ""
echo "Or check GitHub issue #93:"
echo "  https://github.com/anchapin/ironclaw/issues/93"
echo ""
echo "To stop auto-merge:"
echo "  kill \$(cat $PID_FILE)"
echo "  rm $PID_FILE"
