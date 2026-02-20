# Performance Testing Guide

**Version:** 1.0
**Last Updated:** 2026-02-19
**Related Issues:** #541

---

## Overview

This guide provides comprehensive instructions for running and interpreting performance tests for LuminaGuard. Performance testing ensures that the system meets its targets for startup time, VM spawning, memory footprint, and tool call latency.

Performance testing is critical for maintaining LuminaGuard's core value proposition: fast, secure agent execution through Just-in-Time (JIT) Micro-VMs.

---

## Performance Targets

### Primary Targets

| Metric | Target | Current Baseline | Status | Priority |
|--------|--------|------------------|--------|----------|
| **Startup Time** | <500ms | ~300ms | ✅ PASS | Critical |
| **VM Spawn Time** | <200ms | 110-120ms | ✅ PASS | Critical |
| **Memory Footprint** | <200MB | ~200MB | ⚠️ Near target | High |
| **Tool Call Latency** | <100ms | 30-80ms | ✅ PASS | High |

### Secondary Targets

| Metric | Target | Current Baseline | Status | Priority |
|--------|--------|------------------|--------|----------|
| **CPU Utilization** | <50% | 30-80% | ⚠️ Variable | Medium |
| **Network Latency** | <50ms median | 30-80ms | ⚠️ Variable | Medium |
| **Throughput** | 1000+ ops/min | TBD | ⏳ Not measured | Medium |

### Target Definitions

#### Startup Time (<500ms)
- **Definition:** Time from orchestrator process start to first agent request being processed
- **Measurement:** Time from `cargo run` to first tool execution completion
- **Components:** VM pool warmup, MCP client initialization, agent loop startup

#### VM Spawn Time (<200ms)
- **Definition:** Time from spawn request to VM being ready for task execution
- **Measurement:** Time between `vm::spawn_vm()` call and VM handle return
- **Components:** Firecracker startup, kernel loading, rootfs mount, jailer setup
- **Optimization:** Snapshot pooling (target: 10-50ms with pool)

#### Memory Footprint (<200MB)
- **Definition:** Peak RSS memory usage during standard agent workload
- **Measurement:** Maximum resident set size during test execution
- **Components:** Orchestrator binary, Python interpreter, agent state, VM overhead
- **Optimization:** Memory pooling, lazy loading, resource limits

#### Tool Call Latency (<100ms)
- **Definition:** Round-trip time from tool request to response delivery
- **Measurement:** Time from MCP client `call_tool()` to result receipt
- **Components:** Network I/O, MCP protocol overhead, tool execution time
- **Optimization:** Connection pooling, async I/O, efficient serialization

---

## Benchmark Setup

### Prerequisites

#### Python Benchmarks

```bash
# Install psutil for system metrics
cd agent
source .venv/bin/activate
pip install psutil pytest pytest-benchmark
```

#### Rust Benchmarks

```bash
# Install criterion for advanced benchmarking
cd orchestrator
cargo install cargo-criterion

# Build release version for accurate performance measurements
cargo build --release
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

**Note:** Without test assets, benchmarks run in simulation mode. This is acceptable for regression testing but does not reflect real-world performance.

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `LUMINAGUARD_POOL_SIZE` | `5` | Number of snapshots in VM pool (1-20) |
| `LUMINAGUARD_SNAPSHOT_REFRESH_SECS` | `3600` | Snapshot refresh interval in seconds |
| `LUMINAGUARD_SNAPSHOT_PATH` | `/var/lib/luminaguard/snapshots` | Snapshot storage location |
| `RUN_INTEGRATION_TESTS` | `0` | Set to `1` to run integration benchmarks |

---

## Measurement Methodology

### Benchmarking Tools

#### Criterion (Rust)

**Purpose:** Statistical benchmarking framework for Rust

**Features:**
- Statistical analysis (median, mean, standard deviation)
- Automatic outlier detection
- Comparison with previous runs
- HTML report generation

**Usage:**
```bash
cd orchestrator
cargo bench --bench performance_baseline

# Generate HTML report
cargo criterion --open
```

**Output:**
- Detailed statistics in `target/criterion/`
- Comparison with baseline
- Plots and histograms

#### pytest-benchmark (Python)

**Purpose:** Benchmarking plugin for pytest

**Features:**
- Automatic warmup and calibration
- Statistical analysis
- Timer resolution optimization
- JSON/CSV export

**Usage:**
```bash
cd agent
source .venv/bin/activate
pytest tests/performance/agent_benchmarks.py --benchmark-only

# Save results to JSON
pytest tests/performance/agent_benchmarks.py --benchmark-only --benchmark-json=results.json
```

#### Custom Benchmark Harness

**Purpose:** Orchestrator-specific metrics collection

**Features:**
- VM spawn time measurement
- Memory profiling
- CPU utilization tracking
- Network latency measurement

**Usage:**
```bash
cd orchestrator
cargo run --bin performance_benchmark --release

# Quick mode (10 iterations)
cargo run --bin performance_benchmark --release -- --quick
```

---

## Running Benchmarks

### Quick Start

#### Option 1: Run All Benchmarks (Recommended)

```bash
# Run comprehensive benchmarks (100 iterations each)
./scripts/run-performance-benchmarks.sh

# Run quick benchmarks (10 iterations each)
./scripts/run-performance-benchmarks.sh --quick
```

#### Option 2: Run Individual Benchmarks

##### Python Agent Benchmarks

```bash
# Run all Python benchmarks
source agent/.venv/bin/activate
pytest agent/tests/performance/agent_benchmarks.py -v -s

# Run specific benchmark
pytest agent/tests/performance/agent_benchmarks.py::test_agent_comprehensive -v -s

# Run memory benchmark only
pytest agent/tests/performance/agent_benchmarks.py::test_agent_memory -v -s
```

##### Rust Orchestrator Benchmarks

```bash
# Run all Rust benchmarks
cd orchestrator
cargo run --bin performance_benchmark --release

# Run Criterion benchmarks
cargo bench --bench performance_baseline

# View Criterion report
cargo criterion --open
```

### Detailed Benchmark Execution

#### VM Spawn Time Benchmark

```bash
# Python simulation
pytest agent/tests/performance/agent_benchmarks.py::test_agent_comprehensive -v

# Rust with Criterion
cd orchestrator
cargo bench --bench performance_baseline -- vm_spawn_time

# Rust standalone with real VM assets
cargo run --bin performance_benchmark --release
```

**What it measures:**
- Time to spawn a new Micro-VM
- Impact of snapshot pooling (warm vs. cold spawn)
- Consistency across multiple iterations

#### Memory Footprint Benchmark

```bash
# Python memory benchmark
pytest agent/tests/performance/agent_benchmarks.py::test_agent_memory -v

# Measure memory with tools
ps aux | grep luminaguard
top -p $(pgrep -f luminaguard)
```

**What it measures:**
- Peak RSS memory during agent execution
- Memory growth over time (memory leaks)
- Impact of concurrent agent operations

#### CPU Utilization Benchmark

```bash
# Python CPU benchmark
pytest agent/tests/performance/agent_benchmarks.py::test_agent_cpu -v

# Real-time CPU monitoring
while true; do ps aux | grep luminaguard | awk '{sum+=$3} END {print sum}'; sleep 1; done
```

**What it measures:**
- Average CPU usage during typical workload
- CPU spikes during heavy operations
- Efficiency of async operations

#### Tool Call Latency Benchmark

```bash
# Run comprehensive benchmark with tool timings
pytest agent/tests/performance/agent_benchmarks.py::test_agent_comprehensive -v

# View tool-specific latencies
jq '.tool_execution_times' .beads/metrics/performance/agent_baseline_*.json
```

**What it measures:**
- Round-trip time for each tool type
- Impact of MCP server response time
- Serialization/deserialization overhead

---

## Interpreting Results

### Metrics Files

Results are saved in `.beads/metrics/performance/`:

- `agent_baseline_YYYYMMDD_HHMMSS.json` - Python agent metrics
- `agent_memory_YYYYMMDD_HHMMSS.json` - Memory-specific metrics
- `agent_cpu_YYYYMMDD_HHMMSS.json` - CPU-specific metrics
- `rust_baseline_YYYYMMDD_HHMMSS.json` - Rust orchestrator metrics

### JSON Format

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
  "memory_mb": {
    "median": 195.30,
    "p95": 198.50,
    "p99": 201.20,
    "min": 190.00,
    "max": 205.00,
    "std_dev": 3.20
  },
  "meets_target": true
}
```

### Statistical Measures

#### Median (50th Percentile)
- Middle value when sorted
- Best metric for typical performance
- Less affected by outliers

#### P95 (95th Percentile)
- 95% of runs are faster than this
- Measures worst-case acceptable performance
- Important for SLA considerations

#### P99 (99th Percentile)
- 99% of runs are faster than this
- Measures extreme outliers
- Indicates system stability

#### Standard Deviation
- Measure of consistency
- Lower is better (more predictable)
- High std_dev indicates instability

### Result Interpretation Guide

#### VM Spawn Time

| Median | P95 | Interpretation |
|--------|-----|----------------|
| <100ms | <110ms | ✅ Excellent |
| 100-150ms | 110-180ms | ✅ Good |
| 150-200ms | 180-220ms | ⚠️ Acceptable |
| >200ms | >220ms | ❌ Below target |

**Action if below target:**
- Check if snapshot pool is warm
- Verify Firecracker installation
- Review system resource usage
- Consider increasing pool size

#### Memory Footprint

| Median | P95 | Interpretation |
|--------|-----|----------------|
| <150MB | <170MB | ✅ Excellent |
| 150-200MB | 170-220MB | ✅ Good |
| 200-250MB | 220-280MB | ⚠️ Acceptable |
| >250MB | >280ms | ❌ Below target |

**Action if above target:**
- Check for memory leaks (increasing P99)
- Review Python imports (circular imports)
- Verify VM pool size (too many cached VMs)
- Profile with `memory_profiler`

#### Tool Call Latency

| Median | P95 | Interpretation |
|--------|-----|----------------|
| <50ms | <70ms | ✅ Excellent |
| 50-100ms | 70-120ms | ✅ Good |
| 100-150ms | 120-180ms | ⚠️ Acceptable |
| >150ms | >180ms | ❌ Below target |

**Action if above target:**
- Check MCP server responsiveness
- Verify network connectivity
- Review tool implementation (blocking I/O)
- Enable connection pooling

### Example Analysis

```bash
# Extract median values over time
for file in .beads/metrics/performance/rust_baseline_*.json; do
    echo "$file"
    jq '.spawn_time_ms.median' "$file"
done | paste - - | awk '{print $2, $1}'
```

**Output:**
```
110.20 rust_baseline_20260215_033805.json
112.15 rust_baseline_20260216_101230.json
109.80 rust_baseline_20260217_152100.json
```

**Interpretation:**
- Spawn time varies between 109-112ms
- Consistent within <3ms variation
- No performance regression detected
- All measurements below 200ms target

---

## Local Testing Instructions

### Pre-test Checklist

```bash
# 1. Ensure system is idle (no other heavy workloads)
htop  # Check CPU and memory usage

# 2. Verify dependencies
make install

# 3. Clean build artifacts
make clean
cd orchestrator && cargo clean

# 4. Build release version
cd orchestrator && cargo build --release

# 5. Warm up system
# Run a quick test first to warm up caches
./scripts/run-performance-benchmarks.sh --quick
```

### Running Tests

#### Single-Agent Baseline

```bash
# Run single-agent benchmarks (Week 1-2 targets)
./scripts/run-performance-benchmarks.sh

# Expected results:
# - Spawn time: <200ms
# - Memory: <200MB
# - CPU: <50%
# - Network: <50ms
```

#### Scale Testing

```bash
# Test with 5-10 concurrent agents (Week 3-4)
# Modify ITERATIONS in benchmark files
ITERATIONS=50 ./scripts/run-performance-benchmarks.sh

# Monitor system resources
watch -n 1 'ps aux | grep luminaguard'
```

#### Resource Exhaustion

```bash
# Test with 100+ concurrent agents (Week 5-6)
# This requires modifying the benchmark harness
# See: orchestrator/src/bin/performance_benchmark.rs

# Expected: Graceful degradation, OOM mitigation
```

### Post-test Analysis

```bash
# Compare with baseline
diff <(jq '.spawn_time_ms.median' baseline.json) \
     <(jq '.spawn_time_ms.median' latest.json)

# Generate trend report
python scripts/analyze-performance-trends.py

# Check for regressions
python scripts/check-performance-regression.py \
    --baseline .beads/metrics/performance/week1-2_baseline_summary.md \
    --current .beads/metrics/performance/week3-4_baseline_summary.md
```

---

## Troubleshooting

### Common Issues

#### 1. Missing Firecracker Test Assets

**Symptom:**
```
⚠️  Firecracker test assets not found
Using simulated VM spawn...
```

**Solution:**
```bash
./scripts/download-firecracker-assets.sh

# Or skip real VM tests and use simulation mode
# Results will not reflect real-world performance
```

#### 2. Python psutil Not Found

**Symptom:**
```
ModuleNotFoundError: No module named 'psutil'
```

**Solution:**
```bash
cd agent
source .venv/bin/activate
pip install psutil
```

#### 3. Rust Build Errors

**Symptom:**
```
error: linking with `cc` failed
```

**Solution:**
```bash
cd orchestrator
cargo clean
cargo build --release

# If using musl target
rustup target add x86_64-unknown-linux-musl
```

#### 4. Inconsistent Results

**Symptom:**
```
Median: 110.20ms
P99: 250.50ms (high variance)
```

**Solution:**
```bash
# 1. Ensure system is idle
# 2. Increase iterations for better statistics
ITERATIONS=500 ./scripts/run-performance-benchmarks.sh

# 3. Disable power saving
sudo cpupower frequency-set -g performance

# 4. Pin to specific CPU cores
# Edit benchmark files to use taskset
```

#### 5. Memory Leak Detected

**Symptom:**
```
Median: 195MB
P99: 450MB (increasing over time)
```

**Solution:**
```bash
# Profile memory usage
pip install memory_profiler
python -m memory_profiler agent/loop.py

# Check for unclosed resources
lsof | grep luminaguard

# Review Python imports for circular dependencies
# Use importlib for lazy imports
```

#### 6. High CPU Usage

**Symptom:**
```
Average: 65% (above 50% target)
```

**Solution:**
```bash
# Profile CPU usage
# Edit benchmark to use cProfile
python -m cProfile -o profile.stats agent/loop.py

# Analyze results
python -c "import pstats; pstats.Stats('profile.stats').sort_stats('cumulative').print_stats(20)"

# Optimize hot paths
# Consider using async/await instead of blocking calls
```

#### 7. Network Latency Spikes

**Symptom:**
```
Median: 40ms
P99: 300ms (network issues)
```

**Solution:**
```bash
# Test network connectivity
ping -c 100 <mcp-server>

# Check for DNS resolution delays
time nslookup <mcp-server>

# Verify MCP server is responsive
curl -X POST http://localhost:3000/api/tool/call

# Enable connection pooling in MCP client
# Check: agent/mcp_client.py
```

### Debug Mode

Enable verbose output for debugging:

```bash
# Python benchmarks
pytest agent/tests/performance/agent_benchmarks.py -vv -s --log-cli-level=DEBUG

# Rust benchmarks
RUST_LOG=debug cargo run --bin performance_benchmark --release

# Criterion with debug
cargo bench --bench performance_baseline -- --verbose
```

---

## Best Practices

### Benchmark Design

1. **Warmup Runs:** Always run a few warmup iterations before measuring
   ```python
   # Python
   for _ in range(5):  # Warmup
       benchmark_function()
   for _ in range(100):  # Measure
       benchmark_function()
   ```

2. **Statistical Significance:** Use enough iterations
   - Minimum: 100 iterations
   - Recommended: 500-1000 for production validation
   - Quick checks: 10-20 iterations

3. **Isolation:** Run benchmarks on idle systems
   ```bash
   # Check system load
   uptime

   # Stop background services
   systemctl stop docker  # if not needed
   ```

4. **Reproducibility:** Use deterministic workloads
   - Same test data size
   - Same tool parameters
   - Same network conditions (use local MCP servers when possible)

### Performance Optimization

1. **Snapshot Pooling:** Ensure VM pool is warm
   ```bash
   # Warm up pool on startup
   cd orchestrator
   cargo run --release -- warmup-pool

   # Check pool status
   cargo run --release -- pool-stats
   ```

2. **Connection Pooling:** Reuse MCP connections
   ```python
   # Good: Reuse client
   client = McpClient("filesystem", cmd)
   with client:
       for _ in range(100):
           client.call_tool("read_file", {...})

   # Bad: Create new client each time
   for _ in range(100):
       with McpClient("filesystem", cmd) as client:
           client.call_tool("read_file", {...})
   ```

3. **Async I/O:** Use async operations for network calls
   ```rust
   // Good: Async
   async fn call_tool(&self, name: &str) -> Result<Value> {
       self.client.send_async(name).await?
   }

   // Bad: Blocking
   fn call_tool(&self, name: &str) -> Result<Value> {
       self.client.send_blocking(name)?
   }
   ```

4. **Memory Management:** Avoid unnecessary allocations
   ```python
   # Good: Reuse buffers
   buffer = bytearray(1024 * 1024)
   for _ in range(100):
       process_data(buffer)

   # Bad: Allocate each time
   for _ in range(100):
       buffer = bytearray(1024 * 1024)
       process_data(buffer)
   ```

### Continuous Monitoring

1. **Daily Benchmarks:** Run automated benchmarks
   ```bash
   # Add to crontab
   0 0 * * * cd /path/to/luminaguard && ./scripts/run-performance-benchmarks.sh
   ```

2. **CI/CD Integration:** Add to GitHub Actions
   ```yaml
   - name: Run Performance Benchmarks
     run: |
       ./scripts/run-performance-benchmarks.sh --quick
     continue-on-error: true  # Don't block PRs, just alert
   ```

3. **Alerting:** Set up notifications for regressions
   ```bash
   # Use webhook to alert on regression
   python scripts/alert-on-regression.py \
       --webhook https://hooks.slack.com/... \
       --threshold 10%  # Alert if >10% regression
   ```

---

## References to Existing Validation Docs

- **Testing Strategy:** `docs/testing/testing.md` - Overview of testing philosophy, coverage targets, and TDD workflow
- **Performance Benchmarks:** `docs/validation/performance-benchmarks.md` - Detailed Phase 3 validation targets and testing scenarios
- **Quick Start Guide:** `docs/validation/performance-benchmarks-quickstart.md` - Quick instructions for running benchmarks
- **VM Module:** `docs/snapshot-pool-guide.md` - VM snapshot pooling optimization strategies
- **MCP Client:** `docs/api-guide.md` - MCP client implementation and optimization

---

## Appendix

### Benchmark File Reference

| File | Language | Purpose |
|------|----------|---------|
| `agent/tests/performance/agent_benchmarks.py` | Python | Agent loop and tool benchmarks |
| `orchestrator/src/bin/performance_benchmark.rs` | Rust | Standalone orchestrator benchmarks |
| `orchestrator/benches/performance_baseline.rs` | Rust | Criterion-based benchmarks |
| `scripts/run-performance-benchmarks.sh` | Bash | Benchmark orchestration script |

### Metrics File Format

All metrics files follow this structure:

```json
{
  "timestamp": "ISO 8601 timestamp",
  "iterations": "number of iterations",
  "metric_name": {
    "median": "median value",
    "p95": "95th percentile",
    "p99": "99th percentile",
    "min": "minimum value",
    "max": "maximum value",
    "std_dev": "standard deviation"
  },
  "meets_target": "boolean"
}
```

### Performance Targets Summary

| Week | Target | Focus |
|------|--------|-------|
| Week 1-2 | Baseline | Single-agent baseline |
| Week 3-4 | Scale | 5-10 concurrent agents |
| Week 5-6 | Exhaustion | 100+ concurrent agents |
| Week 7-8 | Chaos | VM kills, network partitions |
| Week 9-10 | Recovery | Error handling and recovery |
| Week 11-12 | Production | Pre-deployment validation |

---

**Document Status:** ✅ Complete
**Last Updated:** 2026-02-19
**Maintainer:** LuminaGuard Team
