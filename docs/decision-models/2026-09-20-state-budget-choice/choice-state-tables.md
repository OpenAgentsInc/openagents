| Rung | Median B | Largest B | Pooled | `action` | `needs_code` | `progress` | `risk` | Refused requests |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| unbudgeted | 11,914 | 20,053 | 12/64 | 2/16 | 3/16 | 3/16 | 4/16 | 9 |
| output 512 | 10,064 | 15,561 | 15/64 | 2/16 | 3/16 | 2/16 | 8/16 | 6 |
| output 256 | 9,240 | 14,781 | 16/64 | 2/16 | 3/16 | 2/16 | 9/16 | 5 |
| commands 3 | 9,240 | 14,781 | 16/64 | 2/16 | 3/16 | 2/16 | 9/16 | 5 |
| turns 8 | 6,766 | 9,945 | 26/64 | 6/16 | 6/16 | 2/16 | 12/16 | 0 |
| turns 6 | 5,649 | 8,021 | 25/64 | 7/16 | 5/16 | 3/16 | 10/16 | 0 |
| turns 4 | 3,546 | 5,473 | 28/64 | 10/16 | 6/16 | 2/16 | 10/16 | 0 |
| turns 6, message 1024 | 3,631 | 4,879 | 24/64 | 6/16 | 6/16 | 3/16 | 9/16 | 0 |
| **production: turns 6, message 768** | 3,106 | 4,100 | 25/64 | 7/16 | 6/16 | 4/16 | 8/16 | 0 |
| turns 6, message 512 | 2,717 | 3,309 | 24/64 | 7/16 | 6/16 | 4/16 | 7/16 | 0 |
| turns 4, message 512 | 1,806 | 2,312 | 24/64 | 9/16 | 6/16 | 3/16 | 6/16 | 0 |

Refused requests counts SDK request failures, each contributing four incorrect
rows. Exact error categories are retained in the JSON summary; an SDK `Other`
category does not identify a more specific server cause.

| Rung | Choice | Base | Jev | Choice − base | Choice − Jev |
| --- | --- | --- | --- | --- | --- |
| unbudgeted | 12/64 | 21/64 | 33/64 | -9/64 | -21/64 |
| output 512 | 15/64 | 25/64 | 32/64 | -10/64 | -17/64 |
| output 256 | 16/64 | 26/64 | 33/64 | -10/64 | -17/64 |
| commands 3 | 16/64 | 26/64 | 36/64 | -10/64 | -20/64 |
| turns 8 | 26/64 | 39/64 | 36/64 | -13/64 | -10/64 |
| turns 6 | 25/64 | 41/64 | 35/64 | -16/64 | -10/64 |
| turns 4 | 28/64 | 38/64 | 34/64 | -10/64 | -6/64 |
| turns 6, message 1024 | 24/64 | 37/64 | 34/64 | -13/64 | -10/64 |
| production: turns 6, message 768 | 25/64 | 38/64 | 32/64 | -13/64 | -7/64 |
| turns 6, message 512 | 24/64 | 40/64 | 33/64 | -16/64 | -9/64 |
| turns 4, message 512 | 24/64 | 35/64 | 36/64 | -11/64 | -12/64 |
