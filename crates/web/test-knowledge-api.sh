#!/bin/bash
# Test script for repo-knowledge and file-knowledge APIs
# Run with: ./test-knowledge-api.sh [base_url] [session_cookie]
#
# Example:
#   ./test-knowledge-api.sh https://openagents-web.openagents.workers.dev "session=abc123"

BASE_URL="${1:-http://localhost:8787}"
COOKIE="${2:-}"

echo "Testing Knowledge APIs at $BASE_URL"
echo "======================================="

if [ -z "$COOKIE" ]; then
    echo "WARNING: No session cookie provided. Auth-required endpoints will fail."
    echo "Usage: $0 <base_url> <session_cookie>"
    echo ""
fi

# Test 1: GET repo-knowledge (should return empty or cached data)
echo ""
echo "TEST 1: GET /api/repo-knowledge/test/repo"
echo "-----------------------------------------"
RESP=$(curl -s -w "\nHTTP_CODE:%{http_code}" \
    -H "Cookie: $COOKIE" \
    "$BASE_URL/api/repo-knowledge/test/repo")
HTTP_CODE=$(echo "$RESP" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESP" | grep -v "HTTP_CODE:")
echo "Status: $HTTP_CODE"
echo "Response: $BODY"
if [ "$HTTP_CODE" == "200" ] || [ "$HTTP_CODE" == "404" ]; then
    echo "PASS"
else
    echo "FAIL - Expected 200 or 404"
fi

# Test 2: POST file-knowledge (save a file)
echo ""
echo "TEST 2: POST /api/file-knowledge/test/repo"
echo "------------------------------------------"
RESP=$(curl -s -w "\nHTTP_CODE:%{http_code}" \
    -X POST \
    -H "Cookie: $COOKIE" \
    -H "Content-Type: application/json" \
    -d '{
        "path": "test.txt",
        "sha": "abc123def456",
        "content_preview": "This is test content",
        "file_type": "file",
        "size": 1234
    }' \
    "$BASE_URL/api/file-knowledge/test/repo")
HTTP_CODE=$(echo "$RESP" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESP" | grep -v "HTTP_CODE:")
echo "Status: $HTTP_CODE"
echo "Response: $BODY"
if [ "$HTTP_CODE" == "200" ]; then
    echo "PASS"
else
    echo "FAIL - Expected 200"
fi

# Test 3: POST file-knowledge with large size (bigint edge case)
echo ""
echo "TEST 3: POST /api/file-knowledge with large size"
echo "-------------------------------------------------"
RESP=$(curl -s -w "\nHTTP_CODE:%{http_code}" \
    -X POST \
    -H "Cookie: $COOKIE" \
    -H "Content-Type: application/json" \
    -d '{
        "path": "large_file.bin",
        "sha": "xyz789",
        "content_preview": "Large file preview",
        "file_type": "file",
        "size": 999999999
    }' \
    "$BASE_URL/api/file-knowledge/test/repo")
HTTP_CODE=$(echo "$RESP" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESP" | grep -v "HTTP_CODE:")
echo "Status: $HTTP_CODE"
echo "Response: $BODY"
if [ "$HTTP_CODE" == "200" ]; then
    echo "PASS"
else
    echo "FAIL - Expected 200"
fi

# Test 4: POST repo-knowledge (save repo insights)
echo ""
echo "TEST 4: POST /api/repo-knowledge/test/repo"
echo "------------------------------------------"
RESP=$(curl -s -w "\nHTTP_CODE:%{http_code}" \
    -X POST \
    -H "Cookie: $COOKIE" \
    -H "Content-Type: application/json" \
    -d '{
        "ai_summary": "This is a test repository for API testing",
        "ai_suggestions": ["Task 1", "Task 2", "Task 3"]
    }' \
    "$BASE_URL/api/repo-knowledge/test/repo")
HTTP_CODE=$(echo "$RESP" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESP" | grep -v "HTTP_CODE:")
echo "Status: $HTTP_CODE"
echo "Response: $BODY"
if [ "$HTTP_CODE" == "200" ]; then
    echo "PASS"
else
    echo "FAIL - Expected 200"
fi

# Test 5: GET repo-knowledge (should now have data)
echo ""
echo "TEST 5: GET /api/repo-knowledge/test/repo (after save)"
echo "------------------------------------------------------"
RESP=$(curl -s -w "\nHTTP_CODE:%{http_code}" \
    -H "Cookie: $COOKIE" \
    "$BASE_URL/api/repo-knowledge/test/repo")
HTTP_CODE=$(echo "$RESP" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESP" | grep -v "HTTP_CODE:")
echo "Status: $HTTP_CODE"
echo "Response: $BODY"
if [ "$HTTP_CODE" == "200" ]; then
    echo "PASS"
else
    echo "FAIL - Expected 200"
fi

echo ""
echo "======================================="
echo "Tests complete"
