#!/usr/bin/env bash
#
# Signal handling examples
# Demonstrates how signals are captured and reported
#

set -euo pipefail

HT="${HT:-ht}"

echo "===================================="
echo "Signal Handling Examples"
echo "===================================="
echo ""

echo "Understanding signal-based exit codes"
echo "--------------------------------------"
echo "When a process is terminated by a signal, the exit code is 128 + signal_number:"
echo ""
echo "  SIGINT  (2):  128 + 2  = 130 (Ctrl-C)"
echo "  SIGKILL (9):  128 + 9  = 137"
echo "  SIGTERM (15): 128 + 15 = 143"
echo ""
echo ""

echo "Example 1: Direct signal to PTY child"
echo "--------------------------------------"
echo "When the direct child process receives a signal, both 'code' and 'signal' are set"
echo ""

# Create a test script
cat > /tmp/ht_signal_direct.sh <<'EOF'
#!/bin/sh
# This script just sleeps
sleep 10
EOF
chmod +x /tmp/ht_signal_direct.sh

# Run in background
$HT --subscribe init,exit /tmp/ht_signal_direct.sh > /tmp/ht_signal_direct_output 2>/dev/null &
HT_PID=$!

# Wait for init
sleep 0.2
CHILD_PID=$(grep -o '"pid":[0-9]*' /tmp/ht_signal_direct_output | head -1 | cut -d: -f2)

# Send SIGKILL
echo "Sending SIGKILL to PID $CHILD_PID..."
kill -KILL $CHILD_PID 2>/dev/null

# Wait
wait $HT_PID 2>/dev/null || true

# Show exit event
echo "Exit event:"
grep '"type":"exit"' /tmp/ht_signal_direct_output || echo "No exit event captured"
echo ""
echo "Expected: {\"code\": 137, \"signal\": 9}"
echo ""

rm -f /tmp/ht_signal_direct.sh /tmp/ht_signal_direct_output
echo ""

echo "Example 2: Subprocess receives signal"
echo "--------------------------------------"
echo "When a subprocess is signaled but the shell exits normally, 'signal' is null"
echo ""

# Create a script that spawns a subprocess
cat > /tmp/ht_signal_subprocess.sh <<'EOF'
#!/bin/sh
# Start a subprocess and wait for it
sh -c 'sleep 10' &
pid=$!
sleep 0.1
kill -TERM $pid
wait $pid
EOF
chmod +x /tmp/ht_signal_subprocess.sh

echo "Running script that kills a subprocess..."
OUTPUT=$($HT --subscribe exit /tmp/ht_signal_subprocess.sh 2>/dev/null)
echo "Exit event:"
echo "$OUTPUT"
echo ""
echo "Note: code=143 (128+15) but signal=null because the shell itself wasn't signaled"
echo ""

rm -f /tmp/ht_signal_subprocess.sh
echo ""

echo "Example 3: Handling different signals"
echo "--------------------------------------"
echo ""

test_signal() {
    local signal_name=$1
    local signal_num=$2
    local expected_code=$3

    cat > /tmp/ht_signal_test_$$.sh <<EOF
#!/bin/sh
sleep 10
EOF
    chmod +x /tmp/ht_signal_test_$$.sh

    $HT --subscribe init,exit /tmp/ht_signal_test_$$.sh > /tmp/ht_signal_output_$$ 2>/dev/null &
    local ht_pid=$!

    sleep 0.2
    local child_pid
    child_pid=$(grep -o '"pid":[0-9]*' /tmp/ht_signal_output_$$ | head -1 | cut -d: -f2)

    echo "  Sending $signal_name (signal $signal_num) to PID $child_pid..."
    kill -$signal_name $child_pid 2>/dev/null || true

    wait $ht_pid 2>/dev/null || true

    local exit_event
    exit_event=$(grep '"type":"exit"' /tmp/ht_signal_output_$$ || echo "")
    if [ -n "$exit_event" ]; then
        local code signal
        code=$(echo "$exit_event" | grep -o '"code":[0-9]*' | cut -d: -f2)
        signal=$(echo "$exit_event" | grep -o '"signal":[0-9]*' | cut -d: -f2)
        echo "  → Exit code: $code, Signal: $signal"

        if [ "$code" = "$expected_code" ] && [ "$signal" = "$signal_num" ]; then
            echo "  ✓ Correct"
        else
            echo "  ✗ Expected code=$expected_code, signal=$signal_num"
        fi
    else
        echo "  ✗ No exit event captured"
    fi

    rm -f /tmp/ht_signal_test_$$.sh /tmp/ht_signal_output_$$
    echo ""
}

test_signal "TERM" "15" "143"
test_signal "KILL" "9" "137"

echo ""

echo "Example 4: Graceful shutdown with trap"
echo "---------------------------------------"
echo "A process can trap signals to perform cleanup before exiting"
echo ""

cat > /tmp/ht_trap_example.sh <<'EOF'
#!/bin/sh

cleanup() {
    echo "Performing cleanup..."
    exit 0
}

trap cleanup TERM INT

echo "Process started, waiting for signal..."
sleep 10
EOF
chmod +x /tmp/ht_trap_example.sh

$HT --subscribe init,output,exit /tmp/ht_trap_example.sh > /tmp/ht_trap_output 2>/dev/null &
HT_PID=$!

sleep 0.3
CHILD_PID=$(grep -o '"pid":[0-9]*' /tmp/ht_trap_output | head -1 | cut -d: -f2)

echo "Sending SIGTERM to process with trap..."
kill -TERM $CHILD_PID 2>/dev/null

wait $HT_PID 2>/dev/null || true

echo ""
echo "Exit event:"
grep '"type":"exit"' /tmp/ht_trap_output || echo "No exit event"
echo ""
echo "Note: Process trapped SIGTERM and exited with code 0 (signal=null)"
echo ""

rm -f /tmp/ht_trap_example.sh /tmp/ht_trap_output
echo ""

echo "All examples completed!"
