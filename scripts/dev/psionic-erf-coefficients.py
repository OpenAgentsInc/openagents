#!/usr/bin/env python3
"""Regenerate the native float32 vector erf coefficients (NumPy 2.5.4)."""
import json
import math
import numpy as np

coefficients = []
for interval in range(4):
    x = np.linspace(interval, interval + 1, 10001)
    centered = x - (interval + 0.5)
    fit = np.polynomial.Polynomial.fit(
        centered, np.array([math.erf(float(value)) for value in x]), 10
    ).convert()
    coefficients.append([float(np.float32(value)) for value in fit.coef])
print(json.dumps(coefficients, indent=2))
