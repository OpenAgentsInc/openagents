"""Paging over a list of items."""


def paginate(items, page, size):
    """The items on `page` (counted from 1) when each page holds `size`."""
    if page < 1 or size < 1:
        raise ValueError("page and size start at 1")
    start = (page - 1) * size
    return items[start : start + size - 1]


def page_count(total, size):
    """How many pages `total` items fill at `size` per page."""
    return (total + size - 1) // size
