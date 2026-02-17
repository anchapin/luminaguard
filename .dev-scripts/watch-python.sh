#!/bin/bash
# Watch Python files and run tests on changes
cd "$(git rev-parse --show-toplevel)/agent"
source .venv/bin/activate
ptw tests/ -- -v
