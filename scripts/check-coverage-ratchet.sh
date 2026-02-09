#!/bin/bash
# Check coverage against ratchet

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

if [ ! -f .coverage-baseline.json ]; then
    echo "⚠️  No coverage baseline found, skipping ratchet check"
    exit 0
fi

# Check Python coverage
if [ -d "agent" ]; then
    echo "🐍 Checking Python coverage ratchet..."
    cd agent
    
    if [ ! -f .venv/bin/pytest ]; then
        echo "⚠️  pytest not found, skipping coverage check"
        exit 0
    fi
    
    .venv/bin/pytest tests/ --cov=loop --cov=tools --cov-report=xml -q
    
    CURRENT=$(python3 <<'PYTHON'
import xml.etree.ElementTree as ET
tree = ET.parse('coverage.xml')
pct = float(tree.getroot().attrib.get('line-rate', 0)) * 100
print(f"{pct:.1f}")
PYTHON
)
    
    RATCHET=$(jq -r '.python_ratchet' ../.coverage-baseline.json)
    
    echo "Current: ${CURRENT}%, Ratchet: ${RATCHET}%"
    
    if (( $(echo "$CURRENT < $RATCHET" | bc -l) )); then
        echo "❌ Coverage ${CURRENT}% < ratchet ${RATCHET}%"
        exit 1
    fi
    
    echo "✅ Coverage meets ratchet"
fi
