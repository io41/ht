#!/usr/bin/env bash
#
# Basic ht usage examples
#

set -euo pipefail

# Find ht binary
HT="${HT:-ht}"

echo "===================================="
echo "Basic ht Usage Examples"
echo "===================================="
echo ""

echo "Example 1: Running a simple command"
echo "------------------------------------"
echo "Command: ht --subscribe exit echo 'Hello, World!'"
echo ""
echo "Output:"
$HT --subscribe exit echo 'Hello, World!'
echo ""
echo ""

echo "Example 2: Capturing exit codes"
echo "--------------------------------"
echo "Command: ht --subscribe exit -- bash -c 'exit 42'"
echo ""
echo "Output:"
$HT --subscribe exit -- bash -c 'exit 42'
echo ""
echo ""

echo "Example 3: Interactive shell with commands"
echo "-------------------------------------------"
echo "This example starts bash and sends it commands via JSON"
echo ""
echo "Command: echo '{\"type\": \"input\", \"payload\": \"echo hello\\r\"}' | ht --subscribe init,output,exit"
echo ""
echo "Output:"
echo '{"type": "input", "payload": "echo hello\r"}' | timeout 1 $HT --subscribe init,output,exit || true
echo ""
echo ""

echo "Example 4: Using sendKeys for keyboard input"
echo "---------------------------------------------"
echo "This sends Ctrl-C to interrupt a sleep command"
echo ""
# Note: This example is more complex and demonstrates programmatic interaction
cat > /tmp/ht_example_keys.sh <<'EOF'
#!/usr/bin/env bash
ht --subscribe init,exit bash -c "sleep 10" &
HT_PID=$!

# Wait for init event
sleep 0.2

# Send Ctrl-C
echo '{"type": "sendKeys", "keys": ["^c"]}' >&${HT_PID}

# Wait for ht to exit
wait $HT_PID 2>/dev/null || true
EOF
chmod +x /tmp/ht_example_keys.sh

echo "See /tmp/ht_example_keys.sh for the full example"
echo ""

echo "Example 5: Terminal resize"
echo "--------------------------"
echo "Command: ht --subscribe resize,exit --size 80x24 echo 'Initial size 80x24'"
echo ""
echo "Output:"
$HT --subscribe resize,exit --size 80x24 echo 'Initial size 80x24'
echo ""
echo ""

echo "All examples completed!"
