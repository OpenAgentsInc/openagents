#!/usr/bin/env python3
import re
import sys

def fix_create_issue_calls(content):
    """Add missing project_id parameter to create_issue calls"""

    # Pattern to match create_issue calls with 7 args (missing project_id)
    pattern = r'(issue::create_issue\(\s*&conn,\s*"[^"]*",\s*(?:None|Some\([^)]+\)),\s*Priority::\w+,\s*IssueType::\w+,\s*(?:None|Some\("[^"]*"\)),\s*)(None,)(\s*\))'

    # Replace with 8 args (add project_id)
    replacement = r'\1\2\n        None,\3'

    return re.sub(pattern, replacement, content, flags=re.MULTILINE)

def process_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Check if file has duplicated content (from Perl script error)
    if content.count('//! Integration tests for autopilot database operations') > 1:
        # Find the midpoint where duplication starts
        lines = content.split('\n')
        # Keep only first half
        midpoint = len(lines) // 2
        content = '\n'.join(lines[:midpoint])

    new_content = fix_create_issue_calls(content)

    if new_content != content:
        with open(filepath, 'w') as f:
            f.write(new_content)
        print(f"Fixed {filepath}")
        return True
    return False

if __name__ == '__main__':
    import glob

    files = glob.glob('/home/christopherdavid/code/openagents/crates/autopilot/tests/*.rs')
    fixed_count = 0

    for filepath in files:
        if process_file(filepath):
            fixed_count += 1

    print(f"\nFixed {fixed_count} files")
