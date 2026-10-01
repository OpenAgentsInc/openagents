+++
v = "openagents.eval-case.v1"
description = "A JavaScript stack trace copied from the browser console, with no code frame: the expression that was undefined is only in the source."
kind = "should-fire"
tags = ["javascript", "stack-trace"]

[run]
allowed_operations = ["read", "write"]
+++

Checkout breaks with this in the console. What is going on?

```
Uncaught TypeError: Cannot read properties of undefined (reading 'discount')
    at cartTotal (web/cart.js:4:35)
    at renderCheckout (web/checkout.js:12:15)
```
