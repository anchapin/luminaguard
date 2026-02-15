# Performance Benchmarks Quick Start Guide

This guide provides quick instructions for running LuminaGuard performance benchmarks.

## Prerequisites

### Python Benchmarks

```bash
# Install psutil for system metrics
source agent/.venv/bin/activate
pip install psutil
```

### Rust Benchmarks

```bash
# Build release version for accurate performance
cd orchestrator
cargo build --release
```

## Running Benchmarks

### Option 1: Run All Benchmarks (Recommended)

```bash
# Run comprehensive benchmarks (100 iterations each)
./scripts/run-performance-benchmarks.sh

# Run quick benchmarks (10 iterations each)
./scripts/run-performance-benchmarks.sh --quick
```

### Option 2: Run Individual Benchmarks

#### Python Agent Benchmarks

```bash
# Run all Python benchmarks
source agent/.venv/bin/activate
python agent/tests/performance/agent_benchmarks.py

# Run specific benchmark
pytest agent/tests/performance/agent_benchmarks.py::test_agent_comprehensive -v -s
```

#### Rust Orchestrator Benchmarks

```bash
# Run all Rust benchmarks
cd orchestrator
cargo run --bin performance_benchmark --release

# Run Criterion benchmarks
cargo bench --bench performance_baseline
```

## Understanding the Results

### Metrics Files

Results are saved in `.beads/metrics/performance/`:

- `agent_baseline_YYYYMMDD_HHMMSS.json` - Python agent metrics
- `rust_baseline_YYYYMMDD_HHMMSS.json` - Rust orchestrator metrics
- `week1-2_baseline_summary.md` - Summary and analysis

### Key Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| VM Spawn Time | <200ms | Median time to spawn a VM |
| Memory Usage | <200MB | Peak memory during workload |
| CPU Usage | <50% | Average CPU utilization |
| Network Latency | <50ms | Median round-trip time |

### Statistical Measures

- **Median:** Middle value (50th percentile)
- **P95:** 95th percentile (5% of runs slower)
- **P99:** 99th percentile (1% of runs slower)
- **Std Dev:** Standard deviation (consistency measure)

## Example Output

```
================================================================
🚀 LuminaGuard Week 1-2 Performance Baseline (Rust)
================================================================

Running 100 iterations for each benchmark...

🧪 Benchmarking VM spawn time (100 iterations)...
  Progress: 20/100
  ...
  Median:   110.20ms
  P95:      111.54ms
  P99:      112.62ms
  Target:   <200ms
  Status:   ✅ PASS

================================================================
💾 Metrics saved to: .beads/metrics/performance/rust_baseline_20260215_033805.json
```

## Troubleshooting

### Missing Firecracker Test Assets

If you see:
```
⚠️  Firecracker test assets not found
```

This is expected for synthetic benchmarks. To run real VM benchmarks:

```bash
./scripts/download-firecracker-assets.sh
```

### Python psutil Not Found

```bash
source agent/.venv/bin/activate
pip install psutil
```

### Rust Build Errors

```bash
cd orchestrator
cargo clean
cargo build --release
```

## Performance Targets Reference

### Week 1-2: Single-Agent Baseline
- Spawn time: <200ms ✅
- Memory: <200MB ✅
- CPU: <50% ✅
- Network: <50ms ✅

### Week 3-4: Scale Testing
- 5-10 concurrent agents
- Linear scaling up to 50 agents
- No resource contention

### Week 5-6: Resource Exhaustion
- 100+ concurrent agents
- Graceful degradation
- OOM mitigation

### Week 7-8: Chaos Engineering
- VM kills
- Network partitions
- Error recovery

## Benchmark Configuration

### Iterations

Adjust iterations for faster/slower benchmarks:

```python
# Edit agent/tests/performance/agent_benchmarks.py
ITERATIONS = 100  # Change to desired number
```

```rust
// Rust: Edit orchestrator/src/bin/performance_benchmark.rs
const ITERATIONS: usize = 100; // Change to desired number
```

### Test Assets

Real VM benchmarks require Firecracker test assets:

```bash
# Download test assets
./scripts/download-firecracker-assets.sh

# Assets will be placed in:
# /tmp/luminaguard-fc-test/vmlinux.bin
# /tmp/luminaguard-fc-test/rootfs.ext4
```

## Analyzing Results

### JSON Format

Each metrics file contains:

```json
{
  "timestamp": "2026-02-15T03:32:00.442927+00:00",
  "iterations": 100,
  "spawn_time_ms": {
    "median": 110.20,
    "p95": 111.54,
    "p99": 112.62,
    "min": 110.16,
    "max": 112.62,
    "std_dev": 0.45
  },
  ...
}
```

### Visualization

To visualize trends over time:

```bash
# Install jq for JSON processing
sudo dnf install jq  # Fedora
sudo apt install jq  # Ubuntu

# Extract median values
cat .beads/metrics/performance/rust_baseline_*.json | jq '.spawn_time_ms.median'
```

## Automation

### CI/CD Integration

Add to `.github/workflows/quality-gates.yml`:

```yaml
- name: Run Performance Benchmarks
  run: |
    ./scripts/run-performance-benchmarks.sh --quick
```

### Scheduled Benchmarks

Run daily benchmarks with cron:

```bash
# Add to crontab -e
0 0 * * * cd /path/to/luminaguard && ./scripts/run-performance-benchmarks.sh
```

## References

- Main benchmark document: `docs/validation/performance-benchmarks.md`
- Baseline summary: `.beads/metrics/performance/week1-2_baseline_summary.md`
- Test files:
  - `agent/tests/performance/agent_benchmarks.py`
  - `orchestrator/src/bin/performance_benchmark.rs`
  - `orchestrator/benches/performance_baseline.rs`

## Support

For issues or questions:
1. Check this guide's Troubleshooting section
2. Review `docs/validation/performance-benchmarks.md`
3. Check metrics files for detailed error messages
4. Review benchmark source code for implementation details

---

*Last Updated: 2026-02-15*
*Version: 1.0*
