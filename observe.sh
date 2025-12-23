#!/bin/bash
# observe.sh - Generate traffic and view observability metrics
# Usage: ./observe.sh [traffic|status|open]

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

APP_PORT=3000
PROMETHEUS_URL="http://localhost:9090"

check_stack() {
    if ! curl -s http://localhost:16686 > /dev/null 2>&1; then
        echo -e "${YELLOW}⚠️  Jaeger not running. Start with: docker compose up -d${NC}"
        exit 1
    fi
    if ! curl -s http://localhost:9090 > /dev/null 2>&1; then
        echo -e "${YELLOW}⚠️  Prometheus not running. Start with: docker compose up -d${NC}"
        exit 1
    fi
}

start_app() {
    if curl -s http://localhost:$APP_PORT/health > /dev/null 2>&1; then
        echo -e "${GREEN}✅ App already running${NC}"
        return
    fi
    
    echo -e "${BLUE}🚀 Starting payments-server...${NC}"
    RUST_LOG=info,payments_app=debug,payments_hex=debug \
    OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317 \
    OTEL_EXPORTER_OTLP_PROTOCOL=grpc \
    OTEL_EXPORTER_OTLP_INSECURE=true \
    OTEL_SERVICE_NAME=payments-service-local \
    DATABASE_URL="postgres://user:password@localhost:5433/payments" \
    cargo run --release -p payments-app > app.log 2>&1 &
    
    for i in {1..30}; do
        if curl -s http://localhost:$APP_PORT/health > /dev/null 2>&1; then
            echo -e "${GREEN}✅ App started${NC}"
            return
        fi
        sleep 1
    done
    echo -e "${YELLOW}⚠️  App failed to start. Check app.log${NC}"
    exit 1
}

generate_traffic() {
    echo -e "${BLUE}💥 Generating traffic...${NC}"
    
    # Clear existing API keys to ensure fresh bootstrap
    docker exec -i payments-postgres psql -U user -d payments -c "TRUNCATE api_keys CASCADE;" > /dev/null 2>&1 || true
    
    # Bootstrap API key
    RESPONSE=$(curl -s -X POST http://localhost:$APP_PORT/api/bootstrap \
        -H "Content-Type: application/json" \
        -d '{"name":"observe-key"}')
    
    API_KEY=$(echo "$RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('api_key',''))" 2>/dev/null)
    
    if [ -z "$API_KEY" ]; then
        echo -e "${YELLOW}⚠️  Failed to get API key. Response: $RESPONSE${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ API Key obtained${NC}"
    
    # Create accounts and transactions
    for i in {1..5}; do
        RESPONSE=$(curl -s -X POST http://localhost:$APP_PORT/api/accounts \
            -H "Authorization: Bearer $API_KEY" \
            -H "Content-Type: application/json" \
            -d "{\"name\":\"user-$i\",\"currency\":\"USD\"}")
        
        ACCT=$(echo "$RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('id',''))" 2>/dev/null)
        
        if [ -z "$ACCT" ]; then
            echo -e "${YELLOW}⚠️  Failed to create account: $RESPONSE${NC}"
            continue
        fi
        
        curl -s -X POST "http://localhost:$APP_PORT/api/transactions/deposit" \
            -H "Authorization: Bearer $API_KEY" \
            -H "Content-Type: application/json" \
            -d "{\"account_id\":\"$ACCT\",\"amount\":1000,\"currency\":\"USD\"}" > /dev/null
        
        curl -s -X POST "http://localhost:$APP_PORT/api/transactions/withdraw" \
            -H "Authorization: Bearer $API_KEY" \
            -H "Content-Type: application/json" \
            -d "{\"account_id\":\"$ACCT\",\"amount\":100,\"currency\":\"USD\"}" > /dev/null
    done
    
    echo -e "${GREEN}✅ Traffic generated (5 accounts, 10 transactions)${NC}"
}

show_status() {
    echo ""
    echo -e "${BLUE}📊 OBSERVABILITY STATUS${NC}"
    echo "─────────────────────────────────────"
    
    # Check call count
    CALLS=$(curl -s --get "$PROMETHEUS_URL/api/v1/query" \
        --data-urlencode "query=sum(traces_span_metrics_calls_total{service_name='payments-service'})" | \
        python3 -c "import sys,json; d=json.load(sys.stdin); print(d['data']['result'][0]['value'][1] if d['data']['result'] else '0')" 2>/dev/null || echo "0")
    
    echo -e "Total Calls (internal): ${GREEN}$CALLS${NC}"
    
    # Show links
    echo ""
    echo -e "${BLUE}🔗 DASHBOARDS${NC}"
    echo "─────────────────────────────────────"
    echo -e "  Jaeger Traces:  ${GREEN}http://localhost:16686${NC}"
    echo -e "  Jaeger Monitor: ${GREEN}http://localhost:16686/monitor${NC}"
    echo -e "  Prometheus:     ${GREEN}http://localhost:9090${NC}"
    echo -e "  Grafana:        ${GREEN}http://localhost:3001${NC}"
    echo ""
}

open_dashboards() {
    echo -e "${BLUE}🌐 Opening dashboards...${NC}"
    open "http://localhost:16686/monitor" 2>/dev/null || xdg-open "http://localhost:16686/monitor" 2>/dev/null
}

# Main
case "${1:-all}" in
    traffic)
        check_stack
        start_app
        generate_traffic
        ;;
    status)
        check_stack
        show_status
        ;;
    open)
        open_dashboards
        ;;
    all|*)
        check_stack
        start_app
        generate_traffic
        sleep 3
        show_status
        ;;
esac
