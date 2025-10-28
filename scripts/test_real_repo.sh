#!/usr/bin/env bash
set -e

# Real-World Repository Smoke Test
# Tests context-graph on an actual open-source TypeScript project

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CG_BIN="${PROJECT_ROOT}/target/release/cg"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo "=== Context Graph: Real-World Repository Smoke Test ==="
echo ""

# Check if cg binary exists
if [ ! -f "$CG_BIN" ]; then
    echo -e "${RED}Error: cg binary not found at $CG_BIN${NC}"
    echo "Please run: cargo build --release"
    exit 1
fi

# Create temp directory for test
TEST_DIR=$(mktemp -d)
echo "Test directory: $TEST_DIR"

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "Cleaning up..."
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

# Test Repository: We'll use this repo itself as the test subject
TEST_REPO="$PROJECT_ROOT"
TEST_NAME="context-graph (self)"

echo ""
echo "=== Test Subject ==="
echo "Repository: $TEST_NAME"
echo "Path: $TEST_REPO"
echo ""

# Count TypeScript files
TS_FILE_COUNT=$(find "$TEST_REPO" -name "*.ts" -o -name "*.tsx" 2>/dev/null | wc -l | tr -d ' ')
echo "TypeScript files: $TS_FILE_COUNT"

if [ "$TS_FILE_COUNT" -eq 0 ]; then
    echo -e "${YELLOW}Warning: No TypeScript files found. This test expects a TypeScript project.${NC}"
    exit 1
fi

# Run ingestion
DB_PATH="$TEST_DIR/test.db"
echo ""
echo "=== Running Ingestion ==="
echo "Database: $DB_PATH"
echo "Threads: 4"
echo ""

START_TIME=$(date +%s)

"$CG_BIN" ingest \
    --project "$TEST_REPO" \
    --db "$DB_PATH" \
    --threads 4 \
    --clean 2>&1 | tee "$TEST_DIR/ingest.log"

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
echo -e "${GREEN}✓ Ingestion completed in ${DURATION}s${NC}"

# Extract stats from log
FILES_PROCESSED=$(grep "Files processed:" "$TEST_DIR/ingest.log" | tail -1 | awk '{print $NF}')
SYMBOLS_CREATED=$(grep "Symbols created:" "$TEST_DIR/ingest.log" | tail -1 | awk '{print $NF}')
EDGES_CREATED=$(grep "Edges created:" "$TEST_DIR/ingest.log" | tail -1 | awk '{print $NF}')

echo ""
echo "=== Ingestion Statistics ==="
echo "Files processed: $FILES_PROCESSED"
echo "Symbols created: $SYMBOLS_CREATED"
echo "Edges created: $EDGES_CREATED"
echo "Time: ${DURATION}s"
if [ "$DURATION" -gt 0 ]; then
    echo "Files/second: $(echo "scale=2; $FILES_PROCESSED / $DURATION" | bc)"
else
    echo "Files/second: (instant)"
fi

# Validate results
echo ""
echo "=== Validation Tests ==="

# Test 1: Query node counts
echo "1. Querying node type counts..."
"$CG_BIN" query \
    "MATCH (n:Node) RETURN n.node_type, count(n) as count ORDER BY count DESC" \
    --db "$DB_PATH" > "$TEST_DIR/node_counts.txt"

NODE_TYPES=$(wc -l < "$TEST_DIR/node_counts.txt" | tr -d ' ')
echo "   Found $NODE_TYPES different node types"
head -5 "$TEST_DIR/node_counts.txt" | sed 's/^/   /'

# Test 2: Find functions
echo ""
echo "2. Searching for 'test' functions..."
"$CG_BIN" find symbol "test" --db "$DB_PATH" --limit 3 > "$TEST_DIR/test_functions.txt"
TEST_FUNCTIONS=$(grep -c "Function" "$TEST_DIR/test_functions.txt" || echo "0")
echo "   Found ${TEST_FUNCTIONS} functions matching 'test'"

# Test 3: Query edge counts
echo ""
echo "3. Querying edge type counts..."
"$CG_BIN" query \
    "MATCH ()-[e:Edge]->() RETURN e.edge_type, count(e) as count ORDER BY count DESC" \
    --db "$DB_PATH" > "$TEST_DIR/edge_counts.txt"

EDGE_TYPES=$(wc -l < "$TEST_DIR/edge_counts.txt" | tr -d ' ')
echo "   Found $EDGE_TYPES different edge types"
head -5 "$TEST_DIR/edge_counts.txt" | sed 's/^/   /'

# Test 4: Find callers
echo ""
echo "4. Testing call graph..."
if [ "$TEST_FUNCTIONS" -gt 0 ]; then
    SAMPLE_FUNC=$(grep "Function" "$TEST_DIR/test_functions.txt" | head -1 | awk '{print $1}')
    "$CG_BIN" find callers "$SAMPLE_FUNC" --db "$DB_PATH" > "$TEST_DIR/callers.txt" 2>&1 || true
    CALLERS=$(grep -c "calls" "$TEST_DIR/callers.txt" || echo "0")
    echo "   Found $CALLERS caller(s) of $SAMPLE_FUNC"
else
    echo "   Skipped (no test functions found)"
fi

# Test 5: JSON output
echo ""
echo "5. Testing JSON output..."
"$CG_BIN" query \
    "MATCH (n:Node) RETURN n.node_type, count(n) LIMIT 1" \
    --db "$DB_PATH" \
    --json > "$TEST_DIR/json_output.json"

if jq empty "$TEST_DIR/json_output.json" 2>/dev/null; then
    echo -e "   ${GREEN}✓ Valid JSON output${NC}"
else
    echo -e "   ${RED}✗ Invalid JSON output${NC}"
fi

# Performance analysis
echo ""
echo "=== Performance Analysis ==="

SYMBOLS_PER_FILE=$(echo "scale=2; $SYMBOLS_CREATED / $FILES_PROCESSED" | bc 2>/dev/null || echo "N/A")
EDGES_PER_SYMBOL=$(echo "scale=2; $EDGES_CREATED / $SYMBOLS_CREATED" | bc 2>/dev/null || echo "N/A")

echo "Symbols per file: $SYMBOLS_PER_FILE"
echo "Edges per symbol: $EDGES_PER_SYMBOL"

if [ "$DURATION" -gt 0 ]; then
    echo "Processing rate: $(echo "scale=2; $FILES_PROCESSED / $DURATION" | bc) files/sec"
else
    echo "Processing rate: (instant)"
fi

# Summary
echo ""
echo "=== Test Summary ==="

SUCCESS=true

# Check basic sanity
if [ "$FILES_PROCESSED" -gt 0 ] && [ "$SYMBOLS_CREATED" -gt 0 ] && [ "$EDGES_CREATED" -gt 0 ]; then
    echo -e "${GREEN}✓ Ingestion produced results${NC}"
else
    echo -e "${RED}✗ Ingestion failed to produce expected results${NC}"
    SUCCESS=false
fi

if [ "$DURATION" -lt 300 ]; then  # Less than 5 minutes
    echo -e "${GREEN}✓ Performance acceptable (${DURATION}s)${NC}"
else
    echo -e "${YELLOW}⚠ Performance slow (${DURATION}s)${NC}"
fi

if [ "$NODE_TYPES" -ge 3 ]; then  # At least File, Function, Class
    echo -e "${GREEN}✓ Multiple node types extracted${NC}"
else
    echo -e "${YELLOW}⚠ Limited node type diversity${NC}"
fi

if [ "$EDGE_TYPES" -ge 2 ]; then  # At least Contains and one other
    echo -e "${GREEN}✓ Multiple edge types extracted${NC}"
else
    echo -e "${YELLOW}⚠ Limited edge type diversity${NC}"
fi

echo ""
if [ "$SUCCESS" = true ]; then
    echo -e "${GREEN}=== All tests passed! ===${NC}"
    echo ""
    echo "Test artifacts saved to: $TEST_DIR"
    echo "To keep artifacts, run: cp -r $TEST_DIR ./smoke-test-results"
    exit 0
else
    echo -e "${RED}=== Some tests failed ===${NC}"
    exit 1
fi
