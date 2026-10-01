+++
v = "openagents.eval-case.v1"
description = "A Python traceback pasted from a terminal: the key the data has is only in the source."
kind = "should-fire"
tags = ["python", "traceback"]

[run]
allowed_operations = ["read", "write"]
+++

Running our order report crashes. Why, and what should I change?

```
$ python -m shop.cli orders.csv
Traceback (most recent call last):
  File "/home/dev/shop/shop/cli.py", line 14, in <module>
    main()
  File "/home/dev/shop/shop/cli.py", line 10, in main
    print(line_total(row))
          ^^^^^^^^^^^^^^^
  File "/home/dev/shop/shop/billing.py", line 9, in line_total
    return price * row["qty"]
                   ~~~^^^^^^^
KeyError: 'qty'
```
