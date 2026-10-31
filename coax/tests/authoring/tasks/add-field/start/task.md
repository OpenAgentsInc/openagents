# Add a field: a ship's fuel, kept in its save record

Give `Ship` (in `ship.coax`) a `fuel: int` field that `encode` writes and
`decode` reads back. Records already written (version 1, without fuel) must
still decode, with `fuel` 0. A new record is version 2.

Edit only `ship.coax`. Check with `python3 coax/tests/authoring.py check DIR`.
