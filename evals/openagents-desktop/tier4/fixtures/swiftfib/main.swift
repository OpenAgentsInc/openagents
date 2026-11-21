var bad = 0
for (n, want) in [(0, 0), (1, 1), (2, 1), (10, 55), (20, 6765)] {
    if fib(n) != want { print("FAIL fib(\(n)) = \(fib(n)), want \(want)"); bad += 1 }
}
if bad == 0 { print("ok") }
exit(Int32(bad))
