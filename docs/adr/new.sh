#!/bin/bash

set -e
set -o pipefail

if [[ -z $1 ]]; then
	echo "Usage: $0 \"<ADR Title>\""
	exit 1
fi

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADR_DIR="${SCRIPT_DIR}"
TEMPLATE_FILE="${ADR_DIR}/template.md"

# Function to find the last ADR number
find_last_adr_number() {
	local max_num=0
	local current_num

	for file in "${ADR_DIR}"/[0-9]*.md; do
		if [[ -f ${file} ]]; then
			filename=$(basename "${file}")
			# Extract number from filename using pattern matching
			if [[ ${filename} =~ ^([0-9]+)- ]]; then
				num="${BASH_REMATCH[1]}"
				# Remove leading zeros
				current_num=$(printf "%s\n" "${num#0}")
				if [[ ${current_num} -gt ${max_num} ]]; then
					max_num=${current_num}
				fi
			fi
		fi
	done

	if [[ ${max_num} -gt 0 ]]; then
		echo "${max_num}"
	fi
}

# Function to create a slug from the title
create_slug() {
	echo "${1}" |
		sed -E 's/[^a-zA-Z0-9]+/-/g' |
		sed -E 's/^-+|-+$//g' |
		tr '[:upper:]' '[:lower:]'
}

# Create ADR directory if it doesn't exist
mkdir -p "${ADR_DIR}"

# Find the last ADR number and increment it
LAST_ADR_NUM=$(
	set -e
	find_last_adr_number
)
if [[ -z ${LAST_ADR_NUM} ]]; then
	NEXT_ADR_NUM=1
else
	NEXT_ADR_NUM=$((LAST_ADR_NUM + 1))
fi

# Format with leading zeros (4-digit)
FORMATTED_ADR_NUM=$(printf "%04d" "${NEXT_ADR_NUM}")

ADR_TITLE="$1"
SLUG=$(
	set -e
	create_slug "${ADR_TITLE}"
)
FILE_NAME="${ADR_DIR}/${FORMATTED_ADR_NUM}-${SLUG}.md"

# Try gh cli first, then git config as fallback
AUTHOR_NAME=$(gh api user --jq '.login' 2>/dev/null || git config user.name 2>/dev/null || echo "Unknown")
AUTHOR_NAME="@${AUTHOR_NAME#@}"
CURRENT_DATE=$(date +%Y-%m-%d || echo "Unknown")

if [[ -f ${FILE_NAME} ]]; then
	echo "File ${FILE_NAME} already exists."
	exit 1
fi

# Create the new ADR from the template by replacing placeholders
# using placeholder names that are less likely to conflict.
if ! sed -e "s/{ADR_NUMBER}/${FORMATTED_ADR_NUM}/g" \
	-e "s/{ADR_TITLE}/${ADR_TITLE}/g" \
	-e "s/{DATE}/${CURRENT_DATE}/g" \
	-e "s/{AUTHOR_NAME}/${AUTHOR_NAME}/g" \
	"${TEMPLATE_FILE}" >"${FILE_NAME}"; then
	echo "Error: Failed to create ADR file from template"
	exit 1
fi

echo "Created new ADR: ${FILE_NAME}"
