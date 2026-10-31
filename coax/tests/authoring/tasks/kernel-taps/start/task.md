# Change a kernel: a five-tap box filter

`kernel.coax` filters a row with a three-tap box (the sample and one neighbour
on each side, edges clamped). Make `box` a five-tap box: two neighbours on each
side, still clamped at the edges, each output the mean of the five.

Edit only `kernel.coax`. Check with `python3 coax/tests/authoring.py check DIR`.
