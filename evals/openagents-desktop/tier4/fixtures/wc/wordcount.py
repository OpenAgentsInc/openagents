# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import argparse


def count(text):
    """Words and lines in text."""
    return len(text.split()), len(text.splitlines())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('file')
    args = ap.parse_args()
    words, lines = count(open(args.file).read())
    print(f'words: {words}')
    print(f'lines: {lines}')


if __name__ == '__main__':
    main()
