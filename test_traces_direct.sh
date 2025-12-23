#!/usr/bin/env bash
set -euo pipefail
APP_PORT=3000
DB_PORT=5433

# Start App Direct to Jaeger (Port 4319 mapped to 4317)
export DATABASE_URL="postgres://user:password@localhost:${DB_PORT}/payments"
export OTEL_EXPORTER_OTLP_ENDPOINT="http://127.0.0.1:4319"
export OTEL_SERVICE_NAME="payments-service-local"
export OTEL_EXPORTER_OTLP_PROTOCOL="grpc"
export OTEL_EXPORTER_OTLP_INSECURE=true
export OTEL_BSP_SCHEDULE_DELAY="1000"

echo "🚀 Starting app direct to Jaeger..."
pkill -f payments-server || true
nohup cargo run --bin payments-server > direct_trace.log 2>&1 &
APP_PID=$!
sleep 10

echo "💥 Generating traffic..."
./scripts/common/run_chaos_tests.sh > /dev/null 2>&1 || true

echo "✅ Traffic done. Waiting for flush..."
sleep 5
pkill -f payments-server
echo "✅ Done."
