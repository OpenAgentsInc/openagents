import sys


def main(argv):
    if len(argv) != 2:
        print("usage: python3 -m tally FILE", file=sys.stderr)
        return 2
    with open(argv[1]) as handle:
        numbers = [int(line) for line in handle if line.strip()]
    # The defect the task describes: the mean divides by one too many.
    print(sum(numbers), sum(numbers) / (len(numbers) + 1))
    return 0


sys.exit(main(sys.argv))
