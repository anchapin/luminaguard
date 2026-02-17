#!/bin/bash
# Watch all files and run tests
echo "Starting integrated development watch..."
echo "Press Ctrl+C to stop"
(cd "$(git rev-parse --show-toplevel)/orchestrator" && cargo watch -x "test --lib --bins") &
RUST_PID=$!
(cd "$(git rev-parse --show-toplevel)/agent" && source .venv/bin/activate && ptw tests/ -- -v) &
PYTHON_PID=$!
trap "kill $RUST_PID $PYTHON_PID" EXIT
wait
