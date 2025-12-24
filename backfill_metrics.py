#!/usr/bin/env python3
"""
Temporary script to backfill metrics from trajectory JSON files.
This bypasses the cargo compilation issue by directly implementing the backfill logic.
"""

import json
import sqlite3
import os
from pathlib import Path
from datetime import datetime

def extract_metrics_from_json(json_path):
    """Extract session and tool call metrics from a trajectory JSON file."""
    with open(json_path, 'r') as f:
        trajectory = json.load(f)

    # Session metrics
    session_id = trajectory.get('session_id', '')
    model = trajectory.get('model', '')
    prompt = trajectory.get('prompt', '')
    started_at = trajectory.get('started_at', '')

    usage = trajectory.get('usage', {})
    tokens_in = usage.get('input_tokens', 0)
    tokens_out = usage.get('output_tokens', 0)
    tokens_cached = usage.get('cache_read_tokens', 0)
    cost_usd = usage.get('cost_usd', 0.0)

    result = trajectory.get('result', {})
    duration_ms = result.get('duration_ms', 0)
    duration_seconds = duration_ms / 1000.0
    success = result.get('success', False)
    issues_completed = result.get('issues_completed', 0)
    apm = result.get('apm')

    # Count tool calls and errors from steps
    tool_calls = 0
    tool_errors = 0
    tool_call_metrics = []
    pending_tools = {}

    for step in trajectory.get('steps', []):
        step_type = step.get('step_type', {})

        if 'ToolCall' in step_type:
            tool_data = step_type['ToolCall']
            tool_name = tool_data.get('tool', '')
            tool_id = tool_data.get('tool_id', '')
            timestamp = step.get('timestamp', '')

            tool_calls += 1
            pending_tools[tool_id] = {
                'tool_name': tool_name,
                'timestamp': timestamp,
                'tokens_in': step.get('tokens_in', 0),
                'tokens_out': step.get('tokens_out', 0),
            }

        elif 'ToolResult' in step_type:
            result_data = step_type['ToolResult']
            tool_id = result_data.get('tool_id', '')
            success = result_data.get('success', True)
            timestamp = step.get('timestamp', '')

            if tool_id in pending_tools:
                tool_info = pending_tools[tool_id]

                # Calculate duration
                try:
                    start_time = datetime.fromisoformat(tool_info['timestamp'].replace('Z', '+00:00'))
                    end_time = datetime.fromisoformat(timestamp.replace('Z', '+00:00'))
                    duration_ms = int((end_time - start_time).total_seconds() * 1000)
                except:
                    duration_ms = 0

                tool_call_metrics.append({
                    'session_id': session_id,
                    'timestamp': timestamp,
                    'tool_name': tool_info['tool_name'],
                    'duration_ms': duration_ms,
                    'success': success,
                    'error_type': None if success else 'unknown',
                    'tokens_in': tool_info.get('tokens_in', 0) or 0,
                    'tokens_out': tool_info.get('tokens_out', 0) or 0,
                })

                if not success:
                    tool_errors += 1

    final_status = 'completed' if success else 'crashed'

    session_metrics = {
        'id': session_id,
        'timestamp': started_at,
        'model': model,
        'prompt': prompt,
        'duration_seconds': duration_seconds,
        'tokens_in': tokens_in,
        'tokens_out': tokens_out,
        'tokens_cached': tokens_cached,
        'cost_usd': cost_usd,
        'issues_claimed': issues_completed,  # Approximation
        'issues_completed': issues_completed,
        'tool_calls': tool_calls,
        'tool_errors': tool_errors,
        'final_status': final_status,
        'apm': apm,
        'source': 'autopilot',
        'messages': result.get('num_turns', 0),
    }

    return session_metrics, tool_call_metrics

def init_database(db_path):
    """Initialize or open the metrics database."""
    conn = sqlite3.connect(db_path)

    # Enable WAL mode and foreign keys
    conn.execute("PRAGMA journal_mode = WAL")
    conn.execute("PRAGMA busy_timeout = 5000")
    conn.execute("PRAGMA foreign_keys = ON")

    # Create tables if they don't exist
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            timestamp TEXT NOT NULL,
            model TEXT NOT NULL,
            prompt TEXT NOT NULL,
            duration_seconds REAL NOT NULL,
            tokens_in INTEGER NOT NULL,
            tokens_out INTEGER NOT NULL,
            tokens_cached INTEGER NOT NULL,
            cost_usd REAL NOT NULL,
            issues_claimed INTEGER NOT NULL,
            issues_completed INTEGER NOT NULL,
            tool_calls INTEGER NOT NULL,
            tool_errors INTEGER NOT NULL,
            final_status TEXT NOT NULL,
            apm REAL,
            source TEXT DEFAULT 'autopilot',
            messages INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tool_calls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            duration_ms INTEGER NOT NULL,
            success INTEGER NOT NULL,
            error_type TEXT,
            tokens_in INTEGER NOT NULL,
            tokens_out INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );
    """)

    conn.commit()
    return conn

def backfill_metrics(logs_root, db_path):
    """Backfill metrics from all trajectory JSON files."""
    logs_root = Path(logs_root)

    print(f"📊 Opening metrics database: {db_path}")
    print(f"🔍 Scanning logs directory: {logs_root}")
    print()

    conn = init_database(db_path)

    # Find all date directories
    date_dirs = [d for d in logs_root.iterdir() if d.is_dir() and len(d.name) == 8 and d.name.isdigit()]
    date_dirs.sort()

    if not date_dirs:
        print(f"⚠ No date directories found in {logs_root}")
        return

    print(f"🔍 Found {len(date_dirs)} date directories")
    print()

    total_imported = 0
    total_skipped = 0
    total_errors = 0

    for dir_idx, date_dir in enumerate(date_dirs, 1):
        print(f"[{dir_idx}/{len(date_dirs)}] Processing directory: {date_dir.name}")

        # Find all JSON files
        json_files = list(date_dir.glob("*.json"))

        if not json_files:
            print(f"  ⚠ No JSON files found")
            continue

        print(f"  🔍 Found {len(json_files)} trajectory files")

        dir_imported = 0
        dir_skipped = 0
        dir_errors = 0

        for i, json_file in enumerate(json_files, 1):
            filename = json_file.name
            print(f"  [{i}/{len(json_files)}] {filename}... ", end='', flush=True)

            try:
                session_metrics, tool_call_metrics = extract_metrics_from_json(json_file)

                # Check if session already exists
                cursor = conn.execute("SELECT id FROM sessions WHERE id = ?", (session_metrics['id'],))
                if cursor.fetchone():
                    print("SKIPPED")
                    dir_skipped += 1
                    continue

                # Insert session
                conn.execute("""
                    INSERT INTO sessions (
                        id, timestamp, model, prompt, duration_seconds,
                        tokens_in, tokens_out, tokens_cached, cost_usd,
                        issues_claimed, issues_completed, tool_calls, tool_errors,
                        final_status, apm, source, messages
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """, (
                    session_metrics['id'],
                    session_metrics['timestamp'],
                    session_metrics['model'],
                    session_metrics['prompt'],
                    session_metrics['duration_seconds'],
                    session_metrics['tokens_in'],
                    session_metrics['tokens_out'],
                    session_metrics['tokens_cached'],
                    session_metrics['cost_usd'],
                    session_metrics['issues_claimed'],
                    session_metrics['issues_completed'],
                    session_metrics['tool_calls'],
                    session_metrics['tool_errors'],
                    session_metrics['final_status'],
                    session_metrics['apm'],
                    session_metrics['source'],
                    session_metrics['messages'],
                ))

                # Insert tool calls
                for tc in tool_call_metrics:
                    conn.execute("""
                        INSERT INTO tool_calls (
                            session_id, timestamp, tool_name, duration_ms,
                            success, error_type, tokens_in, tokens_out
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    """, (
                        tc['session_id'],
                        tc['timestamp'],
                        tc['tool_name'],
                        tc['duration_ms'],
                        1 if tc['success'] else 0,
                        tc['error_type'],
                        tc['tokens_in'],
                        tc['tokens_out'],
                    ))

                conn.commit()

                print(f"✓ ({len(tool_call_metrics)} tools, {session_metrics['tool_errors']} errors)")
                dir_imported += 1

            except Exception as e:
                print(f"✗ {e}")
                dir_errors += 1

        print(f"  📊 Imported: {dir_imported}, Skipped: {dir_skipped}, Errors: {dir_errors}")
        print()

        total_imported += dir_imported
        total_skipped += dir_skipped
        total_errors += dir_errors

    conn.close()

    print("=" * 60)
    print(f"📊 Backfill complete:")
    print(f"  Directories processed: {len(date_dirs)}")
    print(f"  Total imported:        {total_imported}")
    print(f"  Total skipped:         {total_skipped}")
    print(f"  Total errors:          {total_errors}")
    print("=" * 60)

if __name__ == '__main__':
    import sys

    logs_root = sys.argv[1] if len(sys.argv) > 1 else 'docs/logs'
    db_path = sys.argv[2] if len(sys.argv) > 2 else 'autopilot-metrics.db'

    backfill_metrics(logs_root, db_path)
