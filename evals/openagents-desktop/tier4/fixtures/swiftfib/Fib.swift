/// The n-th Fibonacci number: fib(0) = 0, fib(1) = 1.
func fib(_ n: Int) -> Int {
    if n <= 1 { return 1 }
    var a = 0, b = 1
    for _ in 1..<n { (a, b) = (b, a + b) }
    return b
}
