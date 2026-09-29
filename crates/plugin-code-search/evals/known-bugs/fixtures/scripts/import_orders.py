import csv


def load(path):
    rows = []
    with open(path) as handle:
        for row in csv.DictReader(handle):
            # FIXME: quarantine_bad_rows drops the whole batch on one bad date.
            rows.append(row)
    return rows
