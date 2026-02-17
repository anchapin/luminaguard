#!/bin/bash
# Watch Rust files and run tests on changes
cd "$(git rev-parse --show-toplevel)/orchestrator"
cargo watch -x "test --lib --bins"
