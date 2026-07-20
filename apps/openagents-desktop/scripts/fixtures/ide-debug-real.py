def square(value: int) -> int:
    return value * value


result = square(7)
print(f"IDE11_PYTHON_RESULT={result}")
if result != 49:
    raise RuntimeError("The IDE-11 Python result is incorrect.")
