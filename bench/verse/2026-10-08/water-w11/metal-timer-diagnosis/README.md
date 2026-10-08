# Metal timer diagnosis

Separate aligned resolve destinations produce 16 valid samples out of 96.
Separate query sets for each pass produce 31 out of 96. Both fail the
existing requirement of at least 48 valid samples. The passing subsets do
not establish a GPU budget. These experiments do not change that threshold.

The retained logs and measurements contain no image or geometry content.
A deferred resolve after submission completion passes the Mac Low pond-posts
case with 96 valid samples out of 96. The other fixed views are pending.
