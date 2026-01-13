#!/bin/bash
# Doc Audit Script - Check docs for drift from code
# Usage: ./doc_audit.sh [--fix]
#
# Checks:
# 1. Backticked file paths that don't exist
# 2. Referenced Rust types (pub struct/trait) that don't exist
# 3. Code fences with File: comments referencing missing files
# 4. Missing doc headers

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DOCS_DIR="$SCRIPT_DIR/../docs"
CRATES_ROOT="$SCRIPT_DIR/../../.."

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

echo "=== DSPy Doc Audit ==="
echo ""

# Track issues
MISSING_PATHS=0
MISSING_TYPES=0
MISSING_DOC_HEADERS=0
TOTAL_DOCS=0

# Check for doc headers
check_headers() {
    echo "Checking doc headers..."
    for doc in "$DOCS_DIR"/*.md; do
        TOTAL_DOCS=$((TOTAL_DOCS + 1))
        filename=$(basename "$doc")

        # Skip README.md check for header format (it has a different style)
        if ! grep -q "^> \*\*Status:\*\*" "$doc" 2>/dev/null; then
            echo -e "${YELLOW}  ⚠ Missing header: $filename${NC}"
            MISSING_DOC_HEADERS=$((MISSING_DOC_HEADERS + 1))
        fi
    done
    echo ""
}

# Check file path references
check_paths() {
    echo "Checking file path references..."
    for doc in "$DOCS_DIR"/*.md; do
        filename=$(basename "$doc")

        # Extract backticked paths like `crates/dsrs/src/...`
        paths=$(grep -oE '\`crates/[^`]+\`' "$doc" 2>/dev/null | tr -d '`' | sort -u || true)

        for path in $paths; do
            # Handle paths that reference directories vs files
            full_path="$CRATES_ROOT/$path"
            if [[ ! -e "$full_path" && ! -d "${full_path%/*}" ]]; then
                echo -e "${RED}  ✗ Missing path in $filename: $path${NC}"
                MISSING_PATHS=$((MISSING_PATHS + 1))
            fi
        done
    done
    echo ""
}

# Check Rust type references
check_types() {
    echo "Checking Rust type references..."

    # Build list of actual types in codebase
    ACTUAL_TYPES=$(find "$CRATES_ROOT/crates" -name "*.rs" -exec grep -h "^pub struct \|^pub trait \|^pub enum " {} \; 2>/dev/null | \
        sed 's/pub struct \([A-Za-z_][A-Za-z0-9_]*\).*/\1/' | \
        sed 's/pub trait \([A-Za-z_][A-Za-z0-9_]*\).*/\1/' | \
        sed 's/pub enum \([A-Za-z_][A-Za-z0-9_]*\).*/\1/' | \
        sort -u)

    for doc in "$DOCS_DIR"/*.md; do
        filename=$(basename "$doc")

        # Look for type names mentioned in docs (simplified check)
        # Focus on types explicitly mentioned as "pub struct X" in code fences
        doc_types=$(grep -oE 'pub struct [A-Za-z_][A-Za-z0-9_]*' "$doc" 2>/dev/null | \
            sed 's/pub struct //' | sort -u || true)

        for type in $doc_types; do
            if ! echo "$ACTUAL_TYPES" | grep -q "^${type}$"; then
                echo -e "${YELLOW}  ⚠ Unverified type in $filename: $type${NC}"
                # Don't count as error since doc may be spec-only
            fi
        done
    done
    echo ""
}

# Check File: comments in code fences
check_file_comments() {
    echo "Checking File: comments in code fences..."
    for doc in "$DOCS_DIR"/*.md; do
        filename=$(basename "$doc")

        # Extract File: comments
        file_refs=$(grep -oE '// File: crates/[^[:space:]]+' "$doc" 2>/dev/null | \
            sed 's|// File: ||' | sort -u || true)

        for ref in $file_refs; do
            full_path="$CRATES_ROOT/$ref"
            if [[ ! -f "$full_path" ]]; then
                echo -e "${RED}  ✗ Missing file ref in $filename: $ref${NC}"
                MISSING_PATHS=$((MISSING_PATHS + 1))
            fi
        done
    done
    echo ""
}

# Run all checks
check_headers
check_paths
check_file_comments
check_types

# Summary
echo "=== Summary ==="
echo "Total docs checked: $TOTAL_DOCS"
echo -e "Missing headers: ${MISSING_DOC_HEADERS}"
echo -e "Missing paths: ${MISSING_PATHS}"
echo ""

if [[ $MISSING_PATHS -gt 0 ]]; then
    echo -e "${RED}Doc drift detected!${NC}"
    exit 1
else
    echo -e "${GREEN}All verified paths exist.${NC}"
fi

if [[ $MISSING_DOC_HEADERS -gt 0 ]]; then
    echo -e "${YELLOW}Some docs missing standard headers.${NC}"
fi

echo ""
echo "Run 'git diff' to see doc changes."
