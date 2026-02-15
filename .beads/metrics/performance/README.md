# Performance Metrics Directory

This directory contains performance benchmark results for LuminaGuard.

## Directory Structure

```
.beads/metrics/performance/
├── README.md                              # This file
├── agent_baseline_YYYYMMDD_HHMMSS.json    # Python agent metrics
├── rust_baseline_YYYYMMDD_HHMMSS.json     # Rust orchestrator metrics
└── week1-2_baseline_summary.md            # Week 1-2 analysis summary
```

## Metrics Files

### Agent Baseline Metrics

File: `agent_baseline_YYYYMMDD_HHMMSS.json`

Contains Python agent performance metrics:
- Agent loop iteration time
- Tool execution times (read_file, write_file, search, list_directory, execute_command)
- Memory usage
- CPU utilization

Example:
```json
{
  "timestamp": "2026-02-15T03:32:00.442927+00:00",
  "iterations": 100,
  "iteration_time": {
    "tool_name": "agent_loop",
    "median_ms": 0.00018,
    "p95_ms": 0.00024,
    "p99_ms": 0.00124,
    "min_ms": 0.00017,
    "max_ms": 0.00124,
    "std_dev_ms": 0.000135
  },
  "tool_execution_times": { ... },
  "memory_mb": 28.98,
  "cpu_percent": 0.0
}
```

### Rust Baseline Metrics

File: `rust_baseline_YYYYMMDD_HHMMSS.json`

Contains Rust orchestrator performance metrics:
- VM spawn time
- Memory operations
- CPU operations
- Network latency

Example:
```json
{
  "timestamp": "2026-02-15T03:38:05.209149689+00:00",
  "iterations": 100,
  "spawn_time_ms": {
    "median": 110.20,
    "p95": 111.54,
    "p99": 112.62,
    "min": 110.16,
    "max": 112.62,
    "std_dev": 0.45
  },
  "memory_mb": { ... },
  "cpu_percent": { ... },
  "network_latency_ms": { ... }
}
```

## Performance Targets

| Metric | Target | Baseline | Status |
|--------|--------|----------|--------|
| VM Spawn Time | <200ms | 110.20ms | ✅ PASS |
| Memory Usage | <200MB | 28.98MB | ✅ PASS |
| CPU Usage | <50% | 0.00% | ✅ PASS |
| Network Latency | <50ms | 10.07ms | ✅ PASS |

## Week 1-2 Baseline Summary

See `week1-2_baseline_summary.md` for comprehensive analysis of Week 1-2 performance baselines.

## Generating New Metrics

To generate new performance metrics:

```bash
# Run all benchmarks
./scripts/run-performance-benchmarks.sh

# Run quick benchmarks (10 iterations)
./scripts/run-performance-benchmarks.sh --quick
```

For detailed instructions, see `docs/validation/performance-benchmarks-quickstart.md`.

## Analyzing Metrics

### View Latest Metrics

```bash
# View latest agent metrics
cat .beads/metrics/performance/agent_baseline_*.json | jq '.'

# View latest Rust metrics
cat .beads/metrics/performance/rust_baseline_*.json | jq '.'
```

### Extract Specific Metrics

```bash
# Get median VM spawn time
cat .beads/metrics/performance/rust_baseline_*.json | jq '.spawn_time_ms.median'

# Get memory usage
cat .beads/metrics/performance/agent_baseline_*.json | jq '.memory_mb'
```

### Compare Multiple Runs

```bash
# List all metrics files
ls -lh .beads/metrics/performance/*.json

# Extract median values for comparison
for f in .beads/metrics/performance/rust_baseline_*.json; do
  echo "$f: $(jq '.spawn_time_ms.median' "$f")ms"
done
```

## Metrics Retention

Metrics files are retained indefinitely for historical tracking and trend analysis. When analyzing trends:

1. Sort files by timestamp
2. Extract key metrics (median, p95)
3. Plot over time to identify performance changes
4. Compare against targets to detect regressions

## CI/CD Integration

Metrics can be integrated into CI/CD pipelines:

```yaml
- name: Run Performance Benchmarks
  run: |
    ./scripts/run-performance-benchmarks.sh --quick

- name: Upload Metrics
  uses: actions/upload-artifact@v3
  with:
    name: performance-metrics
    path: .beads/metrics/performance/*.json
```

## Troubleshooting

### Missing Metrics Files

If metrics files are not generated:

1. Check benchmark execution completed successfully
2. Verify `.beads/metrics/performance/` directory exists
3. Check file permissions

### Invalid JSON Files

If JSON files are corrupted:

1. Validate with `jq`:
   ```bash
   jq '.' .beads/metrics/performance/agent_baseline_*.json
   ```
2. Re-run benchmarks:
   ```bash
   ./scripts/run-performance-benchmarks.sh
   ```

### Outdated Metrics

To update metrics:

```bash
# Remove old metrics
rm .beads/metrics/performance/*.json

# Generate new metrics
./scripts/run-performance-benchmarks.sh
```

## References

- Main benchmark plan: `docs/validation/performance-benchmarks.md`
- Quick start guide: `docs/validation/performance-benchmarks-quickstart.md`
- Week 1-2 summary: `week1-2_baseline_summary.md`

## Contact

For questions about performance metrics:
1. Review documentation in `docs/validation/`
2. Check benchmark source code
3. Consult Week 1-2 summary document

---

*Last Updated: 2026-02-15*
*Version: 1.0*
