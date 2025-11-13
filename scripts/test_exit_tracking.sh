#!/usr/bin/env bash
#
# Validation script for exit code tracking feature
# This script tests that ht correctly captures and reports exit codes and signals
#

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Find ht binary
if [ -f "./target/debug/ht" ]; then
    HT="./target/debug/ht"
elif [ -f "./target/release/ht" ]; then
    HT="./target/release/ht"
elif command -v ht &> /dev/null; then
    HT="ht"
else
    echo -e "${RED}Error: ht binary not found${NC}"
    echo "Please build ht first with: cargo build"
    exit 1
fi

echo "Using ht at: $HT"
echo ""

# Test counter
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to run a test
test_exit_code() {
    local description="$1"
    local command="$2"
    local expected_code="$3"
    local expected_signal="${4:-null}"

    TESTS_RUN=$((TESTS_RUN + 1))
    echo -n "Test $TESTS_RUN: $description... "

    # Create a temporary script
    local script="/tmp/ht_validate_$$.sh"
    echo "#!/bin/sh" > "$script"
    echo "$command" >> "$script"
    chmod +x "$script"

    # Run ht and capture output
    local output
    output=$(timeout 2 "$HT" --subscribe exit "$script" 2>/dev/null || true)

    # Parse JSON output
    local actual_code
    local actual_signal
    actual_code=$(echo "$output" | grep -o '"code":[0-9]*' | head -1 | cut -d: -f2 || echo "")
    actual_signal=$(echo "$output" | grep -o '"signal":[0-9]*\|"signal":null' | head -1 | cut -d: -f2 || echo "")

    # Clean up
    rm -f "$script"

    # Validate results
    if [ "$actual_code" = "$expected_code" ] && [ "$actual_signal" = "$expected_signal" ]; then
        echo -e "${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        echo "  Expected: code=$expected_code, signal=$expected_signal"
        echo "  Got:      code=$actual_code, signal=$actual_signal"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

# Helper function to test signal termination
test_signal() {
    local description="$1"
    local signal_name="$2"
    local signal_num="$3"
    local expected_code="$4"

    TESTS_RUN=$((TESTS_RUN + 1))
    echo -n "Test $TESTS_RUN: $description... "

    # Create a script that sleeps
    local script="/tmp/ht_validate_$$.sh"
    echo "#!/bin/sh" > "$script"
    # Don't trap SIGKILL (it can't be trapped anyway)
    if [ "$signal_name" != "KILL" ]; then
        # For SIGTERM, don't trap it - let it kill the process
        true
    fi
    echo "sleep 10" >> "$script"
    chmod +x "$script"

    # Run ht in background and capture its output
    local output_file="/tmp/ht_output_$$"
    "$HT" --subscribe init,exit "$script" > "$output_file" 2>/dev/null &
    local ht_pid=$!

    # Wait for init event to get the child PID
    local child_pid
    for i in {1..20}; do
        if [ -f "$output_file" ]; then
            child_pid=$(grep -o '"pid":[0-9]*' "$output_file" | head -1 | cut -d: -f2 || true)
            if [ -n "$child_pid" ]; then
                break
            fi
        fi
        sleep 0.1
    done

    if [ -z "$child_pid" ]; then
        echo -e "${RED}FAIL${NC} (couldn't get child PID)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        kill $ht_pid 2>/dev/null || true
        rm -f "$script" "$output_file"
        return 1
    fi

    # Give the trap time to be set up
    sleep 0.2

    # Send signal to the child process
    kill -$signal_name $child_pid 2>/dev/null || true

    # Wait for ht to exit
    wait $ht_pid 2>/dev/null || true

    # Parse output
    local actual_code
    local actual_signal
    actual_code=$(grep -o '"code":[0-9]*' "$output_file" | tail -1 | cut -d: -f2 || echo "")
    actual_signal=$(grep -o '"signal":[0-9]*\|"signal":null' "$output_file" | tail -1 | cut -d: -f2 || echo "")

    # Clean up
    rm -f "$script" "$output_file"

    # Validate results
    if [ "$actual_code" = "$expected_code" ] && [ "$actual_signal" = "$signal_num" ]; then
        echo -e "${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        echo "  Expected: code=$expected_code, signal=$signal_num"
        echo "  Got:      code=$actual_code, signal=$actual_signal"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

echo "========================================="
echo "Exit Code Tracking Validation Tests"
echo "========================================="
echo ""

echo "Testing normal exit codes..."
echo "-----------------------------------------"
test_exit_code "exit 0" "exit 0" "0" "null"
test_exit_code "exit 1" "exit 1" "1" "null"
test_exit_code "exit 42" "exit 42" "42" "null"
test_exit_code "exit 127" "exit 127" "127" "null"
test_exit_code "exit 255" "exit 255" "255" "null"
echo ""

echo "Testing command success/failure..."
echo "-----------------------------------------"
test_exit_code "true command" "true" "0" "null"
test_exit_code "false command" "false" "1" "null"
test_exit_code "nonexistent command" "/nonexistent/command" "127" "null"
echo ""

echo "Testing signal terminations..."
echo "-----------------------------------------"
test_signal "SIGTERM (signal 15)" "TERM" "15" "143"
test_signal "SIGKILL (signal 9)" "KILL" "9" "137"
echo ""

echo "========================================="
echo "Test Summary"
echo "========================================="
echo "Tests run:    $TESTS_RUN"
echo -e "Tests passed: ${GREEN}$TESTS_PASSED${NC}"
if [ $TESTS_FAILED -gt 0 ]; then
    echo -e "Tests failed: ${RED}$TESTS_FAILED${NC}"
    echo ""
    echo -e "${RED}VALIDATION FAILED${NC}"
    exit 1
else
    echo "Tests failed: 0"
    echo ""
    echo -e "${GREEN}ALL TESTS PASSED ✓${NC}"
    exit 0
fi
