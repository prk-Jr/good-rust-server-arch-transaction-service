#!/usr/bin/env bash
#
# 🚀 Run Payments Monitoring Stack
# Starts Infrastructure -> Starts App (Local) -> Generates Traffic -> Verifies Metrics/Traces
#

set -euo pipefail
APP_PORT="${APP_PORT:-3000}"
DB_PORT="${DB_PORT:-5433}"             # Use 5433 to avoid conflict with local postgres
JAEGER_URL="http://localhost:16686"
PROMETHEUS_URL="http://localhost:9090"
GRAFANA_URL="http://localhost:3001"

echo "╔══════════════════════════════════════════════╗"
echo "║      👁️  PAYMENTS MONITORING STACK           ║"
echo "╚══════════════════════════════════════════════╝"

# 1. Start Infrastructure
echo ""
echo "👉 Step 1: Starting Observability Infrastructure (Docker)..."
# Using 5433 for DB to allow local app connection without conflict
if grep -q "5432:5432" docker-compose.yml; then
    sed -i.bak 's/"5432:5432"/"5433:5432"/' docker-compose.yml
    echo "   (Adjusted docker-compose DB port to 5433)"
fi
docker compose up -d db jaeger otel-collector prometheus grafana
echo "✅ Infrastructure Ready"

# 2. Start Application Locally (if not running)
echo ""
echo "👉 Step 2: Checking Application..."

# Check if OTel Collector is reachable
if ! nc -zv 127.0.0.1 4317 >/dev/null 2>&1; then
    echo "⚠️  Cannot connect to OTel Collector at 127.0.0.1:4317. Tracing might fail."
    echo "   (Make sure docker container is exposing 4317)"
fi

if ! nc -zv localhost "$APP_PORT" >/dev/null 2>&1; then
    echo "   🚀 Starting payments-server locally (to enable tracing)..."
    export DATABASE_URL="postgres://user:password@localhost:${DB_PORT}/payments"
    export OTEL_EXPORTER_OTLP_ENDPOINT="http://127.0.0.1:4317"
    export OTEL_SERVICE_NAME="payments-service-local"
    export OTEL_EXPORTER_OTLP_PROTOCOL="grpc"
    export OTEL_BSP_SCHEDULE_DELAY="1000" # flush faster
    # Force insecure (plaintext) for local gRPC
    export OTEL_EXPORTER_OTLP_INSECURE=true 
    
    # Run in background
    nohup cargo run --bin payments-server > app.log 2>&1 &
    APP_PID=$!
    echo "   ⏳ Waiting for app (PID $APP_PID) to be ready..."
    
    # Wait loop
    count=0
    while ! nc -zv localhost "$APP_PORT" >/dev/null 2>&1; do
        sleep 1
        count=$((count+1))
        if [ $count -ge 30 ]; then
            echo "❌ App failed to start. Check app.log:"
            tail -n 10 app.log
            exit 1
        fi
    done
    echo "✅ App Started on port $APP_PORT"
else
    echo "✅ App already running on port $APP_PORT"
fi

# 3. Generate Traffic
echo ""
echo "👉 Step 3: Generating Traffic (Chaos Tests)..."
export BASE_URL="http://localhost:$APP_PORT"
export CHAOS_ITERATIONS=3
if [ -f "./scripts/common/run_chaos_tests.sh" ]; then
    ./scripts/common/run_chaos_tests.sh > /dev/null 2>&1 || true
    echo "✅ Traffic Generated (Traces sent to OTel Collector)"
else
    echo "⚠️  Chaos script not found, skipping traffic generation."
fi

# 4. Verify Metrics (SpanMetrics)
echo ""
echo "👉 Step 4: Verifying Metrics (SpanMetrics)..."
echo "   Querying Prometheus for 'calls_total'..."
sleep 5 # Allow collector to flush

QUERY='sum(calls_total)'
RESULT=$(curl -s --get "$PROMETHEUS_URL/api/v1/query" --data-urlencode "query=$QUERY" | \
    python3 -c "import sys,json; d=json.load(sys.stdin); print(d['data']['result'][0]['value'][1] if d['data']['result'] else 'N/A')" 2>/dev/null || echo "Error")

if [ "$RESULT" == "N/A" ] || [ "$RESULT" == "Error" ]; then
    echo "⚠️  No metrics found yet. (Wait a few seconds and refresh Prometheus)"
else
    echo "✅ Metrics Flowing! Total Calls Recorded: $RESULT"
fi

# 5. Dashboard Links
echo ""
echo "🎉 Monitoring Stack is Live!"
echo "   ───────────────────────────────────────────────"
echo "   🔍 Jaeger (Traces):     $JAEGER_URL"
echo "      → Look for service: 'payments-service-local'"
echo "      → Check 'Monitor' tab for RED metrics"
echo ""
echo "   📈 Prometheus:          $PROMETHEUS_URL"
echo "      → Graph 'traces_span_metrics_calls_total'"
echo ""
echo "   📊 Grafana:             $GRAFANA_URL"
echo "      (Login: admin/admin)"
echo "   ───────────────────────────────────────────────"
echo ""
