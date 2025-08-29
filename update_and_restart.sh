#!/bin/bash

# Use current directory as project directory
PROJECT_DIR="$(pwd)"

# Load environment variables from .env if present
if [ -f "$PROJECT_DIR/.env" ]; then
    source "$PROJECT_DIR/.env"
fi

# Log directory
LOG_DIR="$PROJECT_DIR/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/server.log"
PID_FILE="$LOG_DIR/server.pid"

# -------------------------------
# 1️⃣ Stop running server (if any)
# -------------------------------
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if ps -p $PID > /dev/null; then
        echo "🛑 Stopping server with PID $PID..."
        kill $PID
        sleep 2
    fi
fi

# -------------------------------
# 2️⃣ Pull latest changes
# -------------------------------
echo "⬇️ Pulling latest changes from Git..."
git pull origin master

# -------------------------------
# 3️⃣ Build the project
# -------------------------------
echo "🛠 Building the project..."
cargo build --release

# -------------------------------
# 4️⃣ Start the server
# -------------------------------
echo "🚀 Starting the server..."
nohup ./target/release/server-side-api >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"

echo "✅ Server restarted successfully"
echo "Logs: $LOG_FILE"
echo "PID: $(cat $PID_FILE)"
