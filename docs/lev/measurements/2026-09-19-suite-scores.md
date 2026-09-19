# Suite scores: `support-v1`

52 items, 26 in the calibration split and 26 in the evaluation split, digest `35dfdf43c43c0360`.

Support-desk judgments authored in this repository. Three families, mixed difficulty on purpose so a calibration map has range to fit. Labels are the author's; they are not drawn from an external dataset.

## jev (hosted)

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| evaluation, raw | 0.96 | 0.099 | 0.026 | 0.117 | 0 | 26 |
| evaluation, admitted maps only | 0.96 | 0.083 | 0.018 | 0.096 | 0 | 26 |

| Family | Fitted on | Raw ECE | Mapped ECE | Raw Brier | Mapped Brier | Accuracy | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `routing` | 12 | 0.044 | 0.035 | 0.008 | 0.001 | 1.00 | admitted: ECE 0.044 to 0.035 and Brier 0.008 to 0.001 on held-out items |
| `severity` | 6 | 0.158 | 0.244 | 0.063 | 0.135 | 0.83 | refused: fitted on 6 items, below the floor of 8 |
| `urgency` | 8 | 0.136 | 0.099 | 0.027 | 0.010 | 1.00 | admitted: ECE 0.136 to 0.099 and Brier 0.027 to 0.010 on held-out items |

## kev-0.5b

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| evaluation, raw | 0.88 | 0.176 | 0.122 | 0.388 | 0 | 26 |
| evaluation, admitted maps only | 0.88 | 0.199 | 0.112 | 0.365 | 0 | 26 |

| Family | Fitted on | Raw ECE | Mapped ECE | Raw Brier | Mapped Brier | Accuracy | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `routing` | 12 | 0.194 | 0.178 | 0.066 | 0.054 | 1.00 | admitted: ECE 0.194 to 0.178 and Brier 0.066 to 0.054 on held-out items |
| `severity` | 6 | 0.227 | 0.251 | 0.191 | 0.175 | 0.67 | refused: fitted on 6 items, below the floor of 8 |
| `urgency` | 8 | 0.273 | 0.208 | 0.153 | 0.139 | 0.88 | admitted: ECE 0.273 to 0.208 and Brier 0.153 to 0.139 on held-out items |

## kev-4b

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| evaluation, raw | 0.77 | 0.188 | 0.149 | 0.464 | 1 | 26 |
| evaluation, admitted maps only | 0.77 | 0.149 | 0.148 | 0.487 | 2 | 26 |

| Family | Fitted on | Raw ECE | Mapped ECE | Raw Brier | Mapped Brier | Accuracy | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `routing` | 12 | 0.137 | 0.118 | 0.124 | 0.148 | 0.83 | refused: ECE 0.137 to 0.118, Brier 0.124 to 0.148; the raw signal was already better |
| `severity` | 6 | 0.308 | 0.256 | 0.124 | 0.096 | 0.67 | refused: fitted on 6 items, below the floor of 8 |
| `urgency` | 8 | 0.265 | 0.089 | 0.204 | 0.203 | 0.75 | admitted: ECE 0.265 to 0.089 and Brier 0.204 to 0.203 on held-out items |

## lev

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| --- | --- | --- | --- | --- | --- | --- |
| evaluation, raw | 0.85 | 0.087 | 0.127 | 0.361 | 0 | 26 |
| evaluation, admitted maps only | 0.85 | 0.087 | 0.127 | 0.361 | 0 | 26 |

| Family | Fitted on | Raw ECE | Mapped ECE | Raw Brier | Mapped Brier | Accuracy | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `routing` | 12 | 0.031 | 0.113 | 0.012 | 0.013 | 1.00 | refused: ECE 0.031 to 0.113, Brier 0.012 to 0.013; the raw signal was already better |
| `severity` | 6 | 0.188 | 0.132 | 0.242 | 0.184 | 0.67 | refused: fitted on 6 items, below the floor of 8 |
| `urgency` | 8 | 0.219 | 0.201 | 0.215 | 0.225 | 0.75 | refused: ECE 0.219 to 0.201, Brier 0.215 to 0.225; the raw signal was already better |

