#!/usr/bin/env python3
"""
Trajectory Analysis Script

This script demonstrates how to analyze autopilot trajectory data
using Python and pandas for external analysis.

Usage:
    python analyze_trajectories.py /path/to/logs/
    python analyze_trajectories.py --compare run1.json run2.json
    python analyze_trajectories.py --stats --export stats.csv
"""

import json
import argparse
import sys
from pathlib import Path
from datetime import datetime
from typing import List, Dict, Any

try:
    import pandas as pd
    import matplotlib.pyplot as plt
except ImportError:
    print("Error: pandas and matplotlib are required")
    print("Install with: pip install pandas matplotlib")
    sys.exit(1)


def load_trajectory(path: Path) -> Dict[str, Any]:
    """Load a trajectory JSON file."""
    with open(path) as f:
        return json.load(f)


def extract_metrics(trajectory: Dict[str, Any]) -> Dict[str, Any]:
    """Extract key metrics from a trajectory."""
    result = trajectory.get('result', {})
    usage = trajectory.get('usage', {})

    return {
        'session_id': trajectory.get('session_id', ''),
        'model': trajectory.get('model', ''),
        'success': result.get('success', False),
        'duration_ms': result.get('duration_ms', 0),
        'num_turns': result.get('num_turns', 0),
        'issues_completed': result.get('issues_completed', 0),
        'input_tokens': usage.get('input_tokens', 0),
        'output_tokens': usage.get('output_tokens', 0),
        'cost_usd': usage.get('cost_usd', 0.0),
        'apm': result.get('apm', 0.0),
    }


def analyze_directory(log_dir: Path) -> pd.DataFrame:
    """Analyze all trajectories in a directory."""
    trajectories = []

    for json_file in log_dir.rglob('*.json'):
        try:
            traj = load_trajectory(json_file)
            metrics = extract_metrics(traj)
            metrics['file'] = str(json_file)
            metrics['timestamp'] = datetime.fromtimestamp(
                traj.get('started_at', 0)
            )
            trajectories.append(metrics)
        except Exception as e:
            print(f"Warning: Failed to process {json_file}: {e}")

    return pd.DataFrame(trajectories)


def compare_trajectories(file1: Path, file2: Path):
    """Compare two trajectory files."""
    traj1 = load_trajectory(file1)
    traj2 = load_trajectory(file2)

    m1 = extract_metrics(traj1)
    m2 = extract_metrics(traj2)

    print("=== Trajectory Comparison ===\n")

    print(f"File 1: {file1.name}")
    print(f"File 2: {file2.name}\n")

    metrics_to_compare = [
        ('Success', 'success'),
        ('Duration (s)', lambda m: m['duration_ms'] / 1000),
        ('Turns', 'num_turns'),
        ('Issues Completed', 'issues_completed'),
        ('Input Tokens', 'input_tokens'),
        ('Output Tokens', 'output_tokens'),
        ('Cost (USD)', 'cost_usd'),
        ('APM', 'apm'),
    ]

    for name, key in metrics_to_compare:
        if callable(key):
            v1 = key(m1)
            v2 = key(m2)
        else:
            v1 = m1[key]
            v2 = m2[key]

        # Calculate difference
        if isinstance(v1, (int, float)):
            diff = v2 - v1
            pct = (diff / v1 * 100) if v1 != 0 else 0
            print(f"{name:20} | {v1:>10.2f} | {v2:>10.2f} | {diff:>+10.2f} ({pct:>+6.1f}%)")
        else:
            print(f"{name:20} | {v1:>10} | {v2:>10}")


def print_statistics(df: pd.DataFrame):
    """Print statistical summary."""
    print("=== Trajectory Statistics ===\n")

    print(f"Total runs: {len(df)}")
    print(f"Successful: {df['success'].sum()} ({df['success'].sum() / len(df) * 100:.1f}%)")
    print()

    numeric_cols = ['duration_ms', 'num_turns', 'issues_completed',
                    'input_tokens', 'output_tokens', 'cost_usd', 'apm']

    stats = df[numeric_cols].describe()
    print(stats)

    print("\n=== Model Breakdown ===\n")
    model_stats = df.groupby('model')[numeric_cols].agg(['mean', 'sum'])
    print(model_stats)


def plot_metrics(df: pd.DataFrame, output_file: str = None):
    """Generate plots for trajectory metrics."""
    fig, axes = plt.subplots(2, 2, figsize=(12, 10))

    # APM over time
    df.plot(x='timestamp', y='apm', ax=axes[0, 0], marker='o')
    axes[0, 0].set_title('APM Over Time')
    axes[0, 0].set_ylabel('Actions Per Minute')

    # Cost distribution
    df['cost_usd'].hist(ax=axes[0, 1], bins=20)
    axes[0, 1].set_title('Cost Distribution')
    axes[0, 1].set_xlabel('Cost (USD)')

    # Success rate by model
    success_by_model = df.groupby('model')['success'].mean()
    success_by_model.plot(kind='bar', ax=axes[1, 0])
    axes[1, 0].set_title('Success Rate by Model')
    axes[1, 0].set_ylabel('Success Rate')

    # Issues completed over time
    df.plot(x='timestamp', y='issues_completed', ax=axes[1, 1], marker='o')
    axes[1, 1].set_title('Issues Completed Over Time')
    axes[1, 1].set_ylabel('Issues')

    plt.tight_layout()

    if output_file:
        plt.savefig(output_file)
        print(f"Plot saved to {output_file}")
    else:
        plt.show()


def main():
    parser = argparse.ArgumentParser(description='Analyze autopilot trajectories')
    parser.add_argument('path', nargs='?', help='Log directory or trajectory file')
    parser.add_argument('--compare', nargs=2, metavar=('FILE1', 'FILE2'),
                        help='Compare two trajectory files')
    parser.add_argument('--stats', action='store_true',
                        help='Print statistics')
    parser.add_argument('--plot', action='store_true',
                        help='Generate plots')
    parser.add_argument('--export', metavar='FILE',
                        help='Export data to CSV')

    args = parser.parse_args()

    if args.compare:
        file1 = Path(args.compare[0])
        file2 = Path(args.compare[1])
        compare_trajectories(file1, file2)
        return

    if not args.path:
        parser.print_help()
        return

    path = Path(args.path)

    if path.is_file():
        # Single file analysis
        traj = load_trajectory(path)
        metrics = extract_metrics(traj)
        print("=== Trajectory Metrics ===\n")
        for key, value in metrics.items():
            print(f"{key:20}: {value}")
    elif path.is_dir():
        # Directory analysis
        df = analyze_directory(path)

        if df.empty:
            print("No trajectory files found")
            return

        if args.stats:
            print_statistics(df)

        if args.plot:
            plot_metrics(df, 'trajectory-plots.png')

        if args.export:
            df.to_csv(args.export, index=False)
            print(f"Data exported to {args.export}")

        if not (args.stats or args.plot or args.export):
            # Default: show brief summary
            print(f"Found {len(df)} trajectories")
            print(f"\nRecent runs:")
            recent = df.sort_values('timestamp', ascending=False).head(10)
            print(recent[['timestamp', 'model', 'apm', 'cost_usd', 'success']])
    else:
        print(f"Error: {path} not found")
        sys.exit(1)


if __name__ == '__main__':
    main()
