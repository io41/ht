#!/usr/bin/env bash
#
# Exit code tracking examples
# Demonstrates how to capture and handle process exit codes
#

set -euo pipefail

HT="${HT:-ht}"

echo "===================================="
echo "Exit Code Tracking Examples"
echo "===================================="
echo ""

echo "Example 1: Successful command (exit 0)"
echo "---------------------------------------"
echo "Command: $HT --subscribe exit true"
echo ""
OUTPUT=$($HT --subscribe exit true 2>/dev/null)
echo "$OUTPUT"
CODE=$(echo "$OUTPUT" | grep -o '"code":[0-9]*' | cut -d: -f2)
echo ""
echo "✓ Captured exit code: $CODE"
echo ""
echo ""

echo "Example 2: Failed command (exit 1)"
echo "-----------------------------------"
echo "Command: $HT --subscribe exit false"
echo ""
OUTPUT=$($HT --subscribe exit false 2>/dev/null)
echo "$OUTPUT"
CODE=$(echo "$OUTPUT" | grep -o '"code":[0-9]*' | cut -d: -f2)
echo ""
echo "✓ Captured exit code: $CODE"
echo ""
echo ""

echo "Example 3: Custom exit code"
echo "----------------------------"
echo "Command: $HT --subscribe exit -- bash -c 'exit 42'"
echo ""
OUTPUT=$($HT --subscribe exit -- bash -c 'exit 42' 2>/dev/null)
echo "$OUTPUT"
CODE=$(echo "$OUTPUT" | grep -o '"code":[0-9]*' | cut -d: -f2)
echo ""
echo "✓ Captured exit code: $CODE"
echo ""
echo ""

echo "Example 4: Signal termination (SIGTERM)"
echo "----------------------------------------"
echo "This example demonstrates capturing signal-based termination"
echo ""

# Create a test script
cat > /tmp/ht_sigterm_test.sh <<'EOF'
#!/bin/sh
sleep 10
EOF
chmod +x /tmp/ht_sigterm_test.sh

# Start ht with the script
$HT --subscribe init,exit /tmp/ht_sigterm_test.sh > /tmp/ht_sigterm_output 2>/dev/null &
HT_PID=$!

# Wait for init event and get the child PID
sleep 0.2
CHILD_PID=$(grep -o '"pid":[0-9]*' /tmp/ht_sigterm_output | head -1 | cut -d: -f2)

# Send SIGTERM to the child
kill -TERM $CHILD_PID 2>/dev/null

# Wait for ht to finish
wait $HT_PID 2>/dev/null || true

# Show the exit event
EXIT_EVENT=$(grep '"type":"exit"' /tmp/ht_sigterm_output || echo "")
if [ -n "$EXIT_EVENT" ]; then
    echo "$EXIT_EVENT"
    CODE=$(echo "$EXIT_EVENT" | grep -o '"code":[0-9]*' | cut -d: -f2)
    SIGNAL=$(echo "$EXIT_EVENT" | grep -o '"signal":[0-9]*' | cut -d: -f2)
    echo ""
    echo "✓ Captured exit code: $CODE (128 + 15)"
    echo "✓ Captured signal: $SIGNAL (SIGTERM)"
else
    echo "Note: Signal termination example requires proper signal handling"
fi

# Cleanup
rm -f /tmp/ht_sigterm_test.sh /tmp/ht_sigterm_output
echo ""
echo ""

echo "Example 5: Parsing exit events with jq"
echo "---------------------------------------"
if command -v jq &> /dev/null; then
    echo "Command: $HT --subscribe exit -- bash -c 'exit 99' | jq '.data.code'"
    echo ""
    CODE=$($HT --subscribe exit -- bash -c 'exit 99' 2>/dev/null | jq -r '.data.code')
    echo "Exit code: $CODE"
else
    echo "Note: Install jq to run this example (https://stedolan.github.io/jq/)"
fi
echo ""
echo ""

echo "Example 6: Conditional logic based on exit code"
echo "------------------------------------------------"
echo "#!/bin/bash"
echo ""
echo "OUTPUT=\$(ht --subscribe exit my-command 2>/dev/null)"
echo "CODE=\$(echo \"\$OUTPUT\" | jq -r '.data.code')"
echo ""
echo "if [ \"\$CODE\" -eq 0 ]; then"
echo "    echo \"Success!\""
echo "else"
echo "    echo \"Failed with code \$CODE\""
echo "fi"
echo ""
echo ""

echo "All examples completed!"
