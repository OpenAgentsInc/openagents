---
id: tool.john-7z-wordlist-recovery
version: 1
kind: tool
title: Recover 7-Zip archive passwords with John the Ripper
summary: >-
  Use John the Ripper's 7z format to test candidate passwords from wordlists,
  then independently extract the target member to verify recovered content.
  Applies when an encrypted 7z archive and a plausible candidate list are
  available.
tags: [john-the-ripper, 7z, password-recovery, wordlist]
applies_when: >-
  A task requires recovering the password of an authorized encrypted 7z
  archive, particularly when the system lacks a 7z command-line extractor or a
  suitable Perl compression module.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - crack-7z-hash
  cites:
    - Openwall, *John the Ripper documentation*, sections “john” and “2john”
    - 7-Zip, *7z Command Line Version User's Guide*, sections on commands and switches
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

John the Ripper Jumbo can test 7z archive passwords against a wordlist, but its format converter may have dependencies absent from a minimal environment. Check for `7z2john.pl` and run it on the archive; if Perl reports a missing compression module, install the package that supplies it or otherwise use a compatible converter. Save the converter's output to a temporary hash file, then invoke John with the `7z` format and a wordlist. John tests candidates in list order; this is useful when the password is likely to be a common word, but an unsuccessful wordlist run does not establish that the password is unrecoverable.

After a candidate is found, do not rely only on the cracking tool's status or candidate output. Use a 7-Zip-compatible extractor with the candidate to extract the intended member to standard output, and compare that content with the required artifact. Be aware that archive-member paths and extracted bytes matter: a line-oriented expected answer can be compared after deliberately normalizing newline differences, while binary content should be checked byte-for-byte.

Source: Openwall, *John the Ripper documentation*, sections “john” and “2john”; 7-Zip, *7z Command Line Version User's Guide*, sections on commands and switches.

## How to check

```sh
7z2john.pl archive.7z > /tmp/archive.hash
john --format=7z --wordlist=/path/to/wordlist /tmp/archive.hash
john --show --format=7z /tmp/archive.hash

# Independently test a recovered candidate by extracting the intended member.
7z x -so -p"$candidate" archive.7z path/inside/archive > /tmp/member
cmp /tmp/member expected-file
```

If newline normalization is intentional, compare normalized text explicitly instead of treating a failed byte comparison as a successful exact match.
