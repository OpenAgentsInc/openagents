#!/bin/bash
# Download all benchmark datasets for RLM paper replication
# Usage: ./scripts/download_datasets.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$PROJECT_ROOT/data"
TMP_DIR="/tmp/rlm-datasets"

echo "=== RLM Paper Dataset Downloader ==="
echo "Data directory: $DATA_DIR"
echo ""

mkdir -p "$TMP_DIR"
mkdir -p "$DATA_DIR/sniah"
mkdir -p "$DATA_DIR/oolong"
mkdir -p "$DATA_DIR/codeqa"

# Check for required tools
command -v python3 >/dev/null 2>&1 || { echo "Error: python3 is required"; exit 1; }
command -v git >/dev/null 2>&1 || { echo "Error: git is required"; exit 1; }

# S-NIAH Dataset
echo "=== Downloading S-NIAH (Needle-in-a-Haystack) ==="
if [ ! -d "$TMP_DIR/niah" ]; then
    git clone --depth 1 https://github.com/gkamradt/LLMTest_NeedleInAHaystack "$TMP_DIR/niah"
else
    echo "  Already downloaded, skipping..."
fi
python3 "$SCRIPT_DIR/convert_niah.py" "$TMP_DIR/niah" "$DATA_DIR/sniah/"
echo "  Done: $DATA_DIR/sniah/sniah.jsonl"
echo ""

# OOLONG Dataset
echo "=== Downloading OOLONG ==="
if [ ! -d "$TMP_DIR/oolong" ]; then
    git clone --depth 1 https://github.com/yale-nlp/OOLONG "$TMP_DIR/oolong" 2>/dev/null || {
        echo "  Note: OOLONG repo may not exist or is private"
        echo "  Generating synthetic OOLONG data instead..."
        python3 "$SCRIPT_DIR/convert_oolong.py" --synthetic "$DATA_DIR/oolong/"
    }
else
    echo "  Already downloaded..."
    python3 "$SCRIPT_DIR/convert_oolong.py" "$TMP_DIR/oolong" "$DATA_DIR/oolong/"
fi
echo "  Done: $DATA_DIR/oolong/"
echo ""

# LongBench CodeQA Dataset
echo "=== Downloading LongBench CodeQA ==="
if command -v huggingface-cli >/dev/null 2>&1; then
    if [ ! -d "$TMP_DIR/longbench" ]; then
        huggingface-cli download THUDM/LongBench --local-dir "$TMP_DIR/longbench" --quiet 2>/dev/null || {
            echo "  Note: Could not download from HuggingFace"
            echo "  Generating synthetic CodeQA data instead..."
            python3 "$SCRIPT_DIR/convert_codeqa.py" --synthetic "$DATA_DIR/codeqa/"
        }
    fi
    if [ -d "$TMP_DIR/longbench" ]; then
        python3 "$SCRIPT_DIR/convert_codeqa.py" "$TMP_DIR/longbench" "$DATA_DIR/codeqa/"
    fi
else
    echo "  huggingface-cli not found, generating synthetic data..."
    python3 "$SCRIPT_DIR/convert_codeqa.py" --synthetic "$DATA_DIR/codeqa/"
fi
echo "  Done: $DATA_DIR/codeqa/codeqa.jsonl"
echo ""

echo "=== Summary ==="
echo "Datasets downloaded to: $DATA_DIR"
ls -la "$DATA_DIR"/*/
echo ""
echo "Run bench-runner with: cargo run -p bench-runner -- --dataset s-niah --method base"
