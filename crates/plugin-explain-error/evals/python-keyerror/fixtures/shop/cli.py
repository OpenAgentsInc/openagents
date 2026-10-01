"""Prints order totals."""
import csv
import sys

from shop.billing import line_total


def main():
    for row in csv.DictReader(open(sys.argv[1])):
        print(line_total(row))


if __name__ == "__main__":
    main()
