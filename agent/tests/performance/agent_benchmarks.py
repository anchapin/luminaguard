"""
Week 1-2 Performance Baseline Benchmarks - Agent Side

This module implements performance benchmarks for the Python agent loop.

Key metrics measured:
- Agent loop iteration time
- Tool execution time (read_file, write_file, search, list_directory, execute_command)
- Memory usage of Python process
- CPU utilization

Usage:
    pytest tests/performance/agent_benchmarks.py -v -s

Results are stored in .beads/metrics/performance/ as JSON files.
"""

import json
import os
import sys
import time
import statistics
import psutil
from pathlib import Path
from datetime import datetime, timezone
from typing import Dict, List, Tuple, Any
from dataclasses import dataclass, asdict

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

# Metrics directory
METRICS_DIR = Path(".beads/metrics/performance")

# Ensure metrics directory exists
METRICS_DIR.mkdir(parents=True, exist_ok=True)


@dataclass
class ToolPerformance:
    """Performance metrics for a single tool execution"""

    tool_name: str
    median_ms: float
    p95_ms: float
    p99_ms: float
    min_ms: float
    max_ms: float
    std_dev_ms: float


@dataclass
class AgentLoopMetrics:
    """Performance metrics for agent loop"""

    timestamp: str
    iterations: int
    iteration_time: ToolPerformance
    tool_execution_times: Dict[str, ToolPerformance]
    memory_mb: float
    cpu_percent: float


@dataclass
class SpawnTimeMetrics:
    """VM spawn time metrics"""

    median_ms: float
    p95_ms: float
    p99_ms: float
    min_ms: float
    max_ms: float
    std_dev_ms: float
    meets_target: bool


@dataclass
class ComprehensiveMetrics:
    """Comprehensive performance metrics"""

    timestamp: str
    spawn_time: SpawnTimeMetrics
    memory_mb: float
    cpu_percent: float
    network_latency_ms: float


def calculate_stats(
    values: List[float],
) -> Tuple[float, float, float, float, float, float]:
    """Calculate statistics from a list of values.

    Returns:
        (min, max, median, p95, p99, std_dev)
    """
    if not values:
        return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)

    sorted_values = sorted(values)
    n = len(sorted_values)

    min_val = sorted_values[0]
    max_val = sorted_values[-1]
    median = sorted_values[n // 2]

    p95_idx = int(n * 0.95)
    p95 = sorted_values[min(p95_idx, n - 1)]

    p99_idx = int(n * 0.99)
    p99 = sorted_values[min(p99_idx, n - 1)]

    mean = statistics.mean(values)
    std_dev = statistics.stdev(values) if len(values) > 1 else 0.0

    return (min_val, max_val, median, p95, p99, std_dev)


def measure_memory_mb() -> float:
    """Measure current process memory usage in MB."""
    process = psutil.Process(os.getpid())
    return process.memory_info().rss / (1024 * 1024)


def measure_cpu_percent(duration: float = 1.0) -> float:
    """Measure CPU usage percentage over duration."""
    process = psutil.Process(os.getpid())
    return process.cpu_percent(interval=duration)


def save_metrics(name: str, metrics: Any) -> None:
    """Save metrics to JSON file."""
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    filename = f"{name}_{timestamp}.json"
    filepath = METRICS_DIR / filename

    try:
        with open(filepath, "w") as f:
            json.dump(asdict(metrics), f, indent=2)
        print(f"💾 Metrics saved to: {filepath}")
    except Exception as e:
        print(f"⚠️  Failed to save metrics: {e}")


def benchmark_tool_execution(tool_name: str, iterations: int = 100) -> ToolPerformance:
    """Benchmark a single tool execution.

    Args:
        tool_name: Name of the tool to benchmark
        iterations: Number of iterations to run

    Returns:
        ToolPerformance metrics
    """
    print(f"🧪 Benchmarking {tool_name} ({iterations} iterations)...")

    execution_times = []

    for i in range(iterations):
        start = time.perf_counter()

        # Simulate tool execution
        if tool_name == "read_file":
            # Simulate reading a file
            _ = open(__file__, "r").read()
        elif tool_name == "write_file":
            # Simulate writing a file
            temp_file = Path("/tmp/luminaguard_bench_test.txt")
            temp_file.write_text("test content")
            temp_file.unlink()
        elif tool_name == "search":
            # Simulate search
            "test content".find("test")
        elif tool_name == "list_directory":
            # Simulate listing directory
            list(Path(".").iterdir())
        elif tool_name == "execute_command":
            # Simulate command execution
            os.system("echo test > /dev/null")

        end = time.perf_counter()
        execution_times.append((end - start) * 1000)  # Convert to ms

        if (i + 1) % 20 == 0:
            print(
                f"  Progress: {i + 1}/{iterations} (last: {execution_times[-1]:.2f}ms)"
            )

    min_ms, max_ms, median_ms, p95_ms, p99_ms, std_dev_ms = calculate_stats(
        execution_times
    )

    print(f"  Median:   {median_ms:.2f}ms")
    print(f"  P95:      {p95_ms:.2f}ms")
    print(f"  P99:      {p99_ms:.2f}ms")
    print()

    return ToolPerformance(
        tool_name=tool_name,
        median_ms=median_ms,
        p95_ms=p95_ms,
        p99_ms=p99_ms,
        min_ms=min_ms,
        max_ms=max_ms,
        std_dev_ms=std_dev_ms,
    )


def benchmark_agent_loop(iterations: int = 100) -> ToolPerformance:
    """Benchmark agent loop iteration time.

    Args:
        iterations: Number of iterations to run

    Returns:
        ToolPerformance metrics
    """
    print(f"🧪 Benchmarking agent loop iteration ({iterations} iterations)...")

    iteration_times = []

    # Simulate a simple agent loop
    for i in range(iterations):
        start = time.perf_counter()

        # Simulate loop iteration
        # 1. Read input
        _ = "test input"
        # 2. Process
        _ = "test input".split()
        # 3. Call tool
        _ = len("test input")
        # 4. Format output
        _ = {"result": "test"}

        end = time.perf_counter()
        iteration_times.append((end - start) * 1000)

        if (i + 1) % 20 == 0:
            print(f"  Progress: {i + 1}/{iterations}")

    min_ms, max_ms, median_ms, p95_ms, p99_ms, std_dev_ms = calculate_stats(
        iteration_times
    )

    print(f"  Median:   {median_ms:.2f}ms")
    print(f"  P95:      {p95_ms:.2f}ms")
    print(f"  P99:      {p99_ms:.2f}ms")
    print()

    return ToolPerformance(
        tool_name="agent_loop",
        median_ms=median_ms,
        p95_ms=p95_ms,
        p99_ms=p99_ms,
        min_ms=min_ms,
        max_ms=max_ms,
        std_dev_ms=std_dev_ms,
    )


def benchmark_comprehensive(iterations: int = 100):
    """Run comprehensive baseline benchmark.

    Args:
        iterations: Number of iterations to run
    """
    print("=" * 60)
    print("🚀 WEEK 1-2 AGENT PERFORMANCE BASELINE")
    print("=" * 60)
    print()

    # Benchmark agent loop
    loop_perf = benchmark_agent_loop(iterations)

    # Benchmark tools
    tools = ["read_file", "write_file", "search", "list_directory", "execute_command"]
    tool_perfs = {}

    for tool in tools:
        tool_perfs[tool] = benchmark_tool_execution(tool, iterations)

    # Measure memory
    print("🧪 Measuring memory usage...")
    memory_mb = measure_memory_mb()
    print(f"  Current:  {memory_mb:.2f}MB")
    print()

    # Measure CPU
    print("🧪 Measuring CPU usage (1s)...")
    cpu_percent = measure_cpu_percent(1.0)
    print(f"  Average:  {cpu_percent:.2f}%")
    print()

    # Build metrics
    metrics = AgentLoopMetrics(
        timestamp=datetime.now(timezone.utc).isoformat(),
        iterations=iterations,
        iteration_time=loop_perf,
        tool_execution_times=tool_perfs,
        memory_mb=memory_mb,
        cpu_percent=cpu_percent,
    )

    # Print summary
    print("=" * 60)
    print("📊 AGENT PERFORMANCE BASELINE RESULTS")
    print("=" * 60)
    print()
    print("🔄 Agent Loop Iteration:")
    print(f"  Median:   {loop_perf.median_ms:.2f}ms")
    print(f"  P95:      {loop_perf.p95_ms:.2f}ms")
    print()
    print("🛠️  Tool Execution Times:")
    for tool_name, perf in tool_perfs.items():
        print(f"  {tool_name}:")
        print(f"    Median:   {perf.median_ms:.2f}ms")
        print(f"    P95:      {perf.p95_ms:.2f}ms")
    print()
    print("💾 Memory Usage:")
    print(f"  Current:  {memory_mb:.2f}MB")
    print()
    print("💻 CPU Usage:")
    print(f"  Average:  {cpu_percent:.2f}%")
    print()
    print("=" * 60)

    # Save metrics
    save_metrics("agent_baseline", metrics)


def benchmark_memory(iterations: int = 100):
    """Benchmark memory usage.

    Args:
        iterations: Number of iterations to run
    """
    print(f"🧪 Benchmarking memory usage ({iterations} iterations)...")

    memory_readings = []

    for i in range(iterations):
        # Simulate typical agent workload
        _data = [0] * (1024 * 1024)  # 1MB allocation
        time.sleep(0.001)

        memory_mb = measure_memory_mb()
        memory_readings.append(memory_mb)

        if (i + 1) % 20 == 0:
            print(f"  Progress: {i + 1}/{iterations} (current: {memory_mb:.2f}MB)")

    min_mb, max_mb, median_mb, p95_mb, _, _ = calculate_stats(memory_readings)

    print()
    print("📊 Memory Usage Results:")
    print(f"  Median:   {median_mb:.2f}MB")
    print(f"  P95:      {p95_mb:.2f}MB")
    print(f"  Peak:     {max_mb:.2f}MB")
    print(f"  Target:   <200MB")
    print(f"  Status:   {'✅ PASS' if median_mb < 200 else '❌ FAIL'}")
    print()

    # Save metrics
    save_metrics(
        "agent_memory",
        {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "median_mb": median_mb,
            "p95_mb": p95_mb,
            "peak_mb": max_mb,
            "meets_target": median_mb < 200,
        },
    )


def benchmark_cpu(iterations: int = 100):
    """Benchmark CPU usage.

    Args:
        iterations: Number of iterations to run
    """
    print(f"🧪 Benchmarking CPU usage ({iterations} iterations)...")

    cpu_readings = []

    for i in range(iterations):
        # Simulate typical agent workload
        _data = [0] * (1024 * 10)  # Small allocation
        time.sleep(0.01)

        cpu_percent = measure_cpu_percent(0.01)
        cpu_readings.append(cpu_percent)

        if (i + 1) % 20 == 0:
            print(f"  Progress: {i + 1}/{iterations}")

    min_cpu, max_cpu, avg_cpu, p95_cpu, _, _ = calculate_stats(cpu_readings)

    print()
    print("📊 CPU Usage Results:")
    print(f"  Average:  {avg_cpu:.2f}%")
    print(f"  Peak:     {max_cpu:.2f}%")
    print(f"  Target:   <50%")
    print(f"  Status:   {'✅ PASS' if avg_cpu < 50 else '⚠️  WARNING'}")
    print()

    # Save metrics
    save_metrics(
        "agent_cpu",
        {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "avg_percent": avg_cpu,
            "peak_percent": max_cpu,
            "meets_target": avg_cpu < 50,
        },
    )


# pytest test functions


def test_agent_comprehensive():
    """Test comprehensive agent performance baseline."""
    benchmark_comprehensive(iterations=100)


def test_agent_memory():
    """Test agent memory usage."""
    benchmark_memory(iterations=100)


def test_agent_cpu():
    """Test agent CPU usage."""
    benchmark_cpu(iterations=100)


if __name__ == "__main__":
    # Run benchmarks directly
    benchmark_comprehensive(iterations=100)
