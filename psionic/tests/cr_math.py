#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Correctly rounded sin, cos, exp, log and pow for the Psionic oracles (#12695).

The C library's versions differ between platforms in the last place: glibc's
sin(4) is correctly rounded, macOS's is the next double. An oracle that calls
math.sin therefore prints different references on Linux and the Mac. These
functions return the double nearest the exact value instead, which depends on
nothing but the input, so every platform and Python version gets the same bits.

Each value is computed in decimal arithmetic (Python's decimal and fractions
modules, pure software) with an error bound, then rounded to binary64 by
float(Decimal), which is correctly rounded. If the bound straddles a rounding
boundary the precision doubles and the value is recomputed (Ziv's method).
Nothing here mirrors psionic/lib/strict-math.coax: that is fdlibm's algorithm
and is only within one ulp; this is the exact answer, rounded once.
"""
import math
from decimal import Decimal, localcontext
from fractions import Fraction

_START, _LIMIT = 40, 1280
_PI = {}


def _pi(prec):
    """pi to prec digits (the decimal module's documented series)."""
    if prec not in _PI:
        with localcontext() as ctx:
            ctx.prec = prec + 2
            three = Decimal(3)
            lasts, t, s, n, na, d, da = 0, three, 3, 1, 0, 0, 24
            while s != lasts:
                lasts = s
                n, na = n + na, na + 8
                d, da = d + da, da + 32
                t = (t * n) / d
                s += t
        _PI[prec] = s
    return _PI[prec]


def _round(f):
    """Ziv's loop: f(prec) returns (value, absolute error bound) as Decimals."""
    prec = _START
    while prec <= _LIMIT:
        with localcontext() as ctx:
            ctx.prec = prec + 10
            v, e = f(prec)
            lo, hi = float(v - e), float(v + e)
        if lo == hi:
            return lo
        prec *= 2
    raise ArithmeticError('no correctly rounded result within %d digits' % _LIMIT)


def _taylor(r, odd):
    """sin(r) (odd) or cos(r) by its series, |r| <= pi; the context sets the precision."""
    term = r if odd else Decimal(1)
    total, k, r2 = term, 1 if odd else 0, r * r
    while True:
        term = -term * r2 / ((k + 1) * (k + 2))
        k += 2
        before = total
        total += term
        if total == before:
            return total


def _trig(x, odd):
    x = float(x)
    if not math.isfinite(x):
        return math.nan
    if x == 0:
        return x if odd else 1.0

    def f(prec):
        X = Decimal(x)  # exact
        two_pi = 2 * _pi(prec + 10 + len(str(int(abs(x)))))
        k = (X / two_pi).to_integral_value()
        r = X - k * two_pi
        # Reduction error is at most |X| digits below prec; the series' terms
        # stay below e^pi, so ten guard digits cover both.
        err = Decimal(10) ** (-prec) * (1 + abs(X))
        return _taylor(r, odd), err
    return _round(f)


def sin(x):
    return _trig(x, True)


def cos(x):
    return _trig(x, False)


def exp(x):
    x = float(x)
    if x == 0:
        return 1.0

    def f(prec):
        v = Decimal(x).exp()  # correctly rounded at the context's precision
        return v, abs(v) * Decimal(10) ** (-prec)
    return _round(f)


def log(x):
    x = float(x)
    if not x > 0:
        raise ValueError('log of %r' % x)
    if x == 1:
        return 0.0

    def f(prec):
        v = Decimal(x).ln()  # correctly rounded at the context's precision
        return v, abs(v) * Decimal(10) ** (-prec)
    return _round(f)


def pow(x, y):
    """x ** y: exact for an integer y; y = 0.5 is math.sqrt (IEEE exact); otherwise x > 0."""
    x, y = float(x), float(y)
    if y == 0 or x == 1:
        return 1.0
    if y == int(y) and abs(y) <= 1024 and (x != 0 or y > 0):
        return float(Fraction(x) ** int(y))
    if not x > 0:
        raise ValueError('pow of %r' % x)
    if y == 0.5:
        return math.sqrt(x)

    def f(prec):
        t = Decimal(y) * Decimal(x).ln()
        v = t.exp()
        # The ln and the product carry relative error 10^-(prec+10) each, which
        # moves the exponent by |t| times that; exp adds its own half ulp.
        return v, abs(v) * Decimal(10) ** (-prec) * (1 + abs(t))
    return _round(f)
