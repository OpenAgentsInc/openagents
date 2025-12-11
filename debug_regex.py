import re

pattern = r'^.*\b(?:(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\b.*\b(\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|(?:30(?!-02))|(?:31(?!-(?:02|04|06|09|11)))))\b(?!.*\b\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|(?:30(?!-02))|(?:31(?!-(?:02|04|06|09|11))))\b)'

test_lines = [
    "2025-01-09 User login from 192.168.0.1",
    "Date before IP 2019-06-15 from 172.16.0.5",
    "2024-11-11 login from 198.51.100.23 at start",
]

for line in test_lines:
    match = re.search(pattern, line, re.MULTILINE)
    print(f"Line: {line}")
    print(f"Match: {match.group(1) if match else 'None'}")

    # Test IP matching separately
    ip_pattern = r'\b(?:(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\b'
    ip_match = re.search(ip_pattern, line)
    print(f"IP match: {ip_match.group() if ip_match else 'None'}")
    print()