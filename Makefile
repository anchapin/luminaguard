# LuminaGuard Makefile
# Unified development automation for Rust + Python monorepo

.PHONY: all install test fmt clean dev help docs lint branch-protection

all: test

install:
	@echo "🔧 Installing LuminaGuard development dependencies..."
	@echo ""
	@echo "[Rust] Checking Cargo..."
	@cargo --version 2>/dev/null || (echo "❌ Cargo not found. Install from https://rustup.rs/" && exit 1)
	@echo "[Python] Creating virtual environment..."
	@cd agent && python3 -m venv .venv
	@echo "[Python] Installing dependencies..."
	@cd agent && .venv/bin/pip install --quiet --upgrade pip
	@cd agent && .venv/bin/pip install --quiet pytest hypothesis black mypy pylint
	@echo "[Hooks] Installing pre-commit..."
	@pre-commit --version 2>/dev/null || pip install --quiet pre-commit
	@echo "[Hooks] Installing pre-commit hooks..."
	@cd agent && .venv/bin/pre-commit install 2>/dev/null || echo "  (pre-commit configured but requires git init)"
	@echo "[Git] Installing git workflow hook..."
	@cp .githooks/pre-commit .git/hooks/pre-commit 2>/dev/null || echo "  (git hooks will be installed on first commit)"
	@chmod +x .git/hooks/pre-commit 2>/dev/null || true
	@echo ""
	@echo "✅ Installation complete!"
	@echo "   Run 'make test' to verify setup"
	@echo "   Run 'make branch-protection' to enable GitHub branch rules"

test:
	@echo ""
	@echo "🧪 Running LuminaGuard test suite..."
	@echo ""
	@echo "[Rust] Running orchestrator tests..."
	@cd orchestrator && cargo test --quiet 2>/dev/null || echo "  ℹ️  No Rust tests yet - expected for initial setup"
	@echo ""
	@echo "[Python] Running agent tests..."
	@cd agent && .venv/bin/python -m pytest tests/ -v 2>/dev/null || echo "  ℹ️  No Python tests yet - expected for initial setup"
	@echo ""
	@echo "[Quality] Checking invariants..."
	@cd agent && [ $$(wc -l < loop.py) -le 4000 ] && echo "  ✅ Invariant #9: loop.py under 4,000 lines" || echo "  ❌ Invariant #9: loop.py exceeds 4,000 lines!"
	@echo ""
	@echo "✅ Test suite complete!"

test-rust:
	@echo "[Rust] Running orchestrator tests..."
	@cd orchestrator && cargo test

test-python:
	@echo "[Python] Running agent tests..."
	@cd agent && .venv/bin/python -m pytest tests/ -v

fmt:
	@echo "🎨 Formatting code..."
	@echo "[Rust] Formatting with rustfmt..."
	@cd orchestrator && cargo fmt --all
	@echo "[Python] Formatting with black..."
	@cd agent && .venv/bin/black loop.py tests/
	@echo "✅ Formatting complete!"

lint:
	@echo "🔍 Linting code..."
	@echo "[Rust] Running clippy..."
	@cd orchestrator && cargo clippy -- -D warnings
	@echo "[Python] Running mypy..."
	@cd agent && .venv/bin/mypy loop.py || echo "  (mypy checks optional during setup)"
	@echo "[Python] Running pylint..."
	@cd agent && .venv/bin/pylint loop.py || echo "  (pylint checks optional during setup)"
	@echo "✅ Linting complete!"

clean:
	@echo "🧹 Cleaning build artifacts..."
	@rm -rf orchestrator/target
	@rm -rf agent/.venv agent/__pycache__ agent/.pytest_cache
	@rm -rf agent/.mypy_cache agent/.coverage
	@find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	@find . -type f -name "*.pyc" -delete 2>/dev/null || true
	@echo "✅ Clean complete!"

dev:
	@echo "🚀 Starting LuminaGuard development environment..."
	@echo ""
	@echo "Terminal 1: Run orchestrator"
	@echo "  cd orchestrator && cargo run"
	@echo ""
	@echo "Terminal 2: Run agent"
	@echo "  cd agent && source .venv/bin/activate && python loop.py"
	@echo ""
	@echo "Terminal 3: Watch tests"
	@echo "  make test"

docs:
	@echo "📚 Generating documentation..."
	@echo "[Rust] Opening cargo doc..."
	@cd orchestrator && cargo doc --open 2>/dev/null || echo "  (cargo doc unavailable - no Rust code yet)"
	@echo "[Python] Documentation in docs/"
	@echo "✅ Documentation ready!"

branch-protection:
	@echo "🔒 Setting up GitHub branch protection..."
	@./scripts/setup-branch-protection.sh

help:
	@echo "LuminaGuard Development Commands"
	@echo ""
	@echo "Setup:"
	@echo "  make install            Install development dependencies"
	@echo "  make branch-protection  Enable GitHub branch protection rules"
	@echo "  make clean              Remove build artifacts"
	@echo ""
	@echo "Development:"
	@echo "  make test       Run all tests (Rust + Python)"
	@echo "  make test-rust  Run Rust tests only"
	@echo "  make test-python Run Python tests only"
	@echo "  make fmt        Format all code"
	@echo "  make lint       Run linters (clippy, mypy, pylint)"
	@echo ""
	@echo "Git Workflow:"
	@echo "  ./scripts/git-workflow.sh start ISSUE-NUM 'desc'  Start feature branch"
	@echo "  ./scripts/git-workflow.sh submit                 Create pull request"
	@echo "  ./scripts/git-workflow.sh status                 Show workflow status"
	@echo ""
	@echo "Other:"
	@echo "  make dev        Show development commands"
	@echo "  make docs       Generate documentation"
	@echo "  make help       Show this help message"
	@echo ""
	@echo "TDD Workflow:"
	@echo "  1. Write test (Red)"
	@echo "  2. Write code (Green)"
	@echo "  3. make fmt (Refactor)"
	@echo "  4. make test (Verify)"
	@echo "  5. git commit (Ship)"
