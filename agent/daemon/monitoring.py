"""
Monitoring System Integration for LuminaGuard Daemon

Integrates with external monitoring systems (Prometheus, DataDog, CloudWatch, etc.)
Provides metrics collection, export, and alerting capabilities.

Part of: luminaguard-0va.6 - Daemon Logging and Monitoring
"""

from __future__ import annotations

import json
import time
import logging
from dataclasses import dataclass, asdict
from typing import Optional, Dict, Any, List, Callable
from enum import Enum
from datetime import datetime, timedelta
import threading
from collections import defaultdict

logger = logging.getLogger(__name__)


class MetricType(Enum):
    """Types of metrics"""

    COUNTER = "counter"
    GAUGE = "gauge"
    HISTOGRAM = "histogram"
    TIMER = "timer"


@dataclass
class Metric:
    """Represents a single metric"""

    name: str
    value: float
    metric_type: MetricType
    labels: Dict[str, str]
    timestamp: float

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary"""
        return {
            "name": self.name,
            "value": self.value,
            "type": self.metric_type.value,
            "labels": self.labels,
            "timestamp": datetime.fromtimestamp(self.timestamp).isoformat(),
        }


class MetricCollector:
    """Collects metrics for export"""

    def __init__(self):
        self._counters: Dict[str, float] = defaultdict(float)
        self._gauges: Dict[str, float] = {}
        self._histograms: Dict[str, List[float]] = defaultdict(list)
        self._timers: Dict[str, List[float]] = defaultdict(list)
        self._lock = threading.Lock()

    def increment_counter(
        self, name: str, value: float = 1.0, labels: Optional[Dict[str, str]] = None
    ) -> None:
        """Increment a counter metric"""
        with self._lock:
            key = self._make_key(name, labels)
            self._counters[key] += value

    def set_gauge(
        self, name: str, value: float, labels: Optional[Dict[str, str]] = None
    ) -> None:
        """Set a gauge metric"""
        with self._lock:
            key = self._make_key(name, labels)
            self._gauges[key] = value

    def record_histogram(
        self, name: str, value: float, labels: Optional[Dict[str, str]] = None
    ) -> None:
        """Record a histogram value"""
        with self._lock:
            key = self._make_key(name, labels)
            self._histograms[key].append(value)

    def record_timer(
        self, name: str, seconds: float, labels: Optional[Dict[str, str]] = None
    ) -> None:
        """Record a timer value (in seconds)"""
        with self._lock:
            key = self._make_key(name, labels)
            self._timers[key].append(seconds)

    def get_metrics(self) -> List[Dict[str, Any]]:
        """Get all collected metrics"""
        with self._lock:
            metrics = []

            # Counters
            for name, value in self._counters.items():
                metrics.append(
                    {
                        "name": name.split(":")[0],
                        "value": value,
                        "type": "counter",
                        "labels": self._parse_labels(name),
                        "timestamp": time.time(),
                    }
                )

            # Gauges
            for name, value in self._gauges.items():
                metrics.append(
                    {
                        "name": name.split(":")[0],
                        "value": value,
                        "type": "gauge",
                        "labels": self._parse_labels(name),
                        "timestamp": time.time(),
                    }
                )

            # Histograms (summary statistics)
            for name, values in self._histograms.items():
                if values:
                    metrics.extend(self._summarize_histogram(name, values))

            # Timers (summary statistics)
            for name, values in self._timers.items():
                if values:
                    metrics.extend(self._summarize_timer(name, values))

            return metrics

    def _make_key(self, name: str, labels: Optional[Dict[str, str]]) -> str:
        """Create a key from metric name and labels"""
        if labels:
            label_str = ",".join(f"{k}={v}" for k, v in sorted(labels.items()))
            return f"{name}:{label_str}"
        return name

    def _parse_labels(self, key: str) -> Dict[str, str]:
        """Parse labels from a composite key"""
        if ":" not in key:
            return {}
        label_str = key.split(":", 1)[1]
        labels = {}
        for pair in label_str.split(","):
            if "=" in pair:
                k, v = pair.split("=", 1)
                labels[k] = v
        return labels

    def _summarize_histogram(
        self, name: str, values: List[float]
    ) -> List[Dict[str, Any]]:
        """Summarize histogram statistics"""
        import statistics

        return [
            {
                "name": f"{name}.count",
                "value": len(values),
                "type": "counter",
                "labels": self._parse_labels(name),
                "timestamp": time.time(),
            },
            {
                "name": f"{name}.sum",
                "value": sum(values),
                "type": "gauge",
                "labels": self._parse_labels(name),
                "timestamp": time.time(),
            },
            {
                "name": f"{name}.min",
                "value": min(values),
                "type": "gauge",
                "labels": self._parse_labels(name),
                "timestamp": time.time(),
            },
            {
                "name": f"{name}.max",
                "value": max(values),
                "type": "gauge",
                "labels": self._parse_labels(name),
                "timestamp": time.time(),
            },
            {
                "name": f"{name}.mean",
                "value": statistics.mean(values),
                "type": "gauge",
                "labels": self._parse_labels(name),
                "timestamp": time.time(),
            },
        ]

    def _summarize_timer(self, name: str, values: List[float]) -> List[Dict[str, Any]]:
        """Summarize timer statistics"""
        return self._summarize_histogram(f"{name}_seconds", values)

    def reset(self) -> None:
        """Reset all metrics"""
        with self._lock:
            self._counters.clear()
            self._gauges.clear()
            self._histograms.clear()
            self._timers.clear()


class MonitoringExporter:
    """Exports metrics to external systems"""

    def __init__(self):
        self._exporters: List[Callable[[List[Dict[str, Any]]], None]] = []

    def add_exporter(self, exporter: Callable[[List[Dict[str, Any]]], None]) -> None:
        """Add an exporter handler"""
        self._exporters.append(exporter)

    def export(self, metrics: List[Dict[str, Any]]) -> None:
        """Export metrics to all registered exporters"""
        for exporter in self._exporters:
            try:
                exporter(metrics)
            except Exception as e:
                logger.error(f"Error exporting metrics: {e}")


class PrometheusExporter:
    """Prometheus metrics format exporter"""

    @staticmethod
    def export(metrics: List[Dict[str, Any]]) -> str:
        """Convert metrics to Prometheus format"""
        lines = []
        for metric in metrics:
            name = metric.get("name", "").replace("-", "_")
            value = metric.get("value", 0)
            labels = metric.get("labels", {})

            if labels:
                label_str = ",".join(f'{k}="{v}"' for k, v in sorted(labels.items()))
                lines.append(f"{name}{{{label_str}}} {value}")
            else:
                lines.append(f"{name} {value}")

        return "\n".join(lines)


class CloudWatchExporter:
    """AWS CloudWatch metrics exporter"""

    def __init__(self, namespace: str, region: str = "us-east-1"):
        self.namespace = namespace
        self.region = region
        self._client = None

    async def export_async(self, metrics: List[Dict[str, Any]]) -> None:
        """Export metrics to CloudWatch (async)"""
        try:
            import boto3

            if not self._client:
                self._client = boto3.client("cloudwatch", region_name=self.region)

            # Group metrics by namespace and dimension combinations
            metric_data = []
            for metric in metrics:
                labels = metric.get("labels", {})
                dimensions = [
                    {"Name": k, "Value": v} for k, v in sorted(labels.items())
                ]

                metric_data.append(
                    {
                        "MetricName": metric.get("name", ""),
                        "Value": metric.get("value", 0),
                        "Unit": "None",
                        "Timestamp": datetime.fromtimestamp(
                            metric.get("timestamp", time.time())
                        ),
                        "Dimensions": dimensions,
                    }
                )

            # Send in batches (CloudWatch limit is 20 per request)
            for i in range(0, len(metric_data), 20):
                self._client.put_metric_data(
                    Namespace=self.namespace, MetricData=metric_data[i : i + 20]
                )
        except Exception as e:
            logger.error(f"Error exporting to CloudWatch: {e}")


class DataDogExporter:
    """Datadog metrics exporter"""

    def __init__(self, api_key: str, app_key: str = None, site: str = "datadoghq.com"):
        self.api_key = api_key
        self.app_key = app_key
        self.site = site

    async def export_async(self, metrics: List[Dict[str, Any]]) -> None:
        """Export metrics to Datadog (async)"""
        try:
            import aiohttp

            series_data = []
            now = int(time.time())

            for metric in metrics:
                labels = metric.get("labels", {})
                tags = [f"{k}:{v}" for k, v in sorted(labels.items())]

                series_data.append(
                    {
                        "metric": metric.get("name", ""),
                        "points": [[now, metric.get("value", 0)]],
                        "tags": tags,
                        "type": "gauge",
                    }
                )

            url = f"https://api.{self.site}/api/v1/series"
            headers = {
                "DD-API-KEY": self.api_key,
                "Content-Type": "application/json",
            }

            async with aiohttp.ClientSession() as session:
                async with session.post(
                    url, json={"series": series_data}, headers=headers
                ) as resp:
                    if resp.status != 200:
                        logger.error(f"Datadog export failed: {resp.status}")
        except Exception as e:
            logger.error(f"Error exporting to Datadog: {e}")


class AlertingSystem:
    """Simple alerting system based on metric thresholds"""

    def __init__(self):
        self._rules: List[AlertRule] = []
        self._alert_handlers: List[Callable[[str, str], None]] = []

    def add_rule(self, rule: AlertRule) -> None:
        """Add an alerting rule"""
        self._rules.append(rule)

    def add_alert_handler(self, handler: Callable[[str, str], None]) -> None:
        """Add an alert handler"""
        self._alert_handlers.append(handler)

    def check_metrics(self, metrics: List[Dict[str, Any]]) -> None:
        """Check metrics against alert rules"""
        for metric in metrics:
            name = metric.get("name", "")
            value = metric.get("value", 0)

            for rule in self._rules:
                if rule.matches(name, value):
                    self._send_alert(rule, metric)

    def _send_alert(self, rule: AlertRule, metric: Dict[str, Any]) -> None:
        """Send an alert"""
        alert_message = (
            f"Alert: {rule.name} - {metric.get('name')} = {metric.get('value')}"
        )
        alert_severity = rule.severity.value

        for handler in self._alert_handlers:
            try:
                handler(alert_severity, alert_message)
            except Exception as e:
                logger.error(f"Error sending alert: {e}")


@dataclass
class AlertRule:
    """Represents an alerting rule"""

    name: str
    metric_pattern: str  # fnmatch pattern
    condition: str  # "greater_than", "less_than", "equals"
    threshold: float
    severity: AlertSeverity

    def matches(self, metric_name: str, value: float) -> bool:
        """Check if metric matches this rule"""
        import fnmatch

        if not fnmatch.fnmatch(metric_name, self.metric_pattern):
            return False

        if self.condition == "greater_than":
            return value > self.threshold
        elif self.condition == "less_than":
            return value < self.threshold
        elif self.condition == "equals":
            return value == self.threshold

        return False


class AlertSeverity(Enum):
    """Alert severity levels"""

    INFO = "info"
    WARNING = "warning"
    CRITICAL = "critical"
