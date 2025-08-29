#!/bin/bash

# Use the current directory as the project directory
PROJECT_DIR="$(pwd)"

# Load environment variables from .env in the current directory
if [ -f "$PROJECT_DIR/.env" ]; then
    source "$PROJECT_DIR/.env"
fi

# Log directory (inside current project folder)
LOG_DIR="$PROJECT_DIR/logs"
mkdir -p "$LOG_DIR"

# Log file
LOG_FILE="$LOG_DIR/server.log"

# Run server in the background
nohup cargo run --release > "$LOG_FILE" 2>&1 &

# Save PID
PID_FILE="$LOG_DIR/server.pid"
echo $! > "$PID_FILE"

echo "🚀 Server started in background"
echo "PID: $(cat $PID_FILE)"
echo "Logs: $LOG_FILE"
