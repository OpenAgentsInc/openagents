import sys


def main(argv):
    if len(argv) != 2:
        print("usage: python3 -m tally FILE", file=sys.stderr)
        return 2
    with open(argv[1]) as handle:
        numbers = [int(line) for line in handle if line.strip()]
    print(sum(numbers), sum(numbers) / len(numbers))
    return 0


sys.exit(main(sys.argv))
