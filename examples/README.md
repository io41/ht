# ht Examples

This directory contains practical examples demonstrating common usage patterns for `ht` (headless terminal).

## Shell Script Examples

### basic_usage.sh
Demonstrates fundamental ht operations:
- Running simple commands
- Capturing exit codes
- Sending JSON commands
- Using sendKeys
- Terminal resize

Run:
```bash
./examples/basic_usage.sh
```

### exit_tracking.sh
Shows how to capture and handle process exit codes:
- Normal exit codes (0, 1, custom codes)
- Exit code parsing
- Conditional logic based on exit codes
- Using jq for JSON parsing

Run:
```bash
./examples/exit_tracking.sh
```

### signal_handling.sh
Demonstrates signal-based process termination:
- Direct signal to PTY child (code and signal both set)
- Subprocess signal handling (code set, signal null)
- Different signal types (SIGTERM, SIGKILL, SIGINT)
- Trap handlers for graceful shutdown

Run:
```bash
./examples/signal_handling.sh
```

## Environment Variables

Set `HT` to specify the path to the ht binary:
```bash
export HT=/path/to/ht
./examples/basic_usage.sh
```

If not set, examples will use `ht` from PATH.

## Learning Path

1. Start with `basic_usage.sh` to understand fundamental operations
2. Explore `exit_tracking.sh` to learn about exit code handling
3. Study `signal_handling.sh` for advanced signal handling

## Contributing

Feel free to add more examples! Useful additions might include:
- Integration with testing frameworks
- WebSocket API usage
- Complex terminal automation scenarios
- Language-specific clients (TypeScript, Go, Rust, etc.)
