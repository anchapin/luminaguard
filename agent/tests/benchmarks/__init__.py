"""
Performance benchmarks for LuminaGuard Agent.

This module contains benchmarks measuring key performance metrics:
- Startup time
- Tool call latency
- Memory footprint
- Message processing latency

Usage:
    pytest tests/benchmarks/ --benchmark-only

Performance Targets (from CLAUDE.md):
- Startup time: <500ms
- Tool call latency: <100ms
- Memory footprint: <200MB baseline
"""
