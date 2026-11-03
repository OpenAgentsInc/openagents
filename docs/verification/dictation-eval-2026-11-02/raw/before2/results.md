## Quality (held 300 ms before speech to 200 ms after; GPU)

| Set | Clips | Words | WER % | CER % | Exact clips |
| --- | ---: | ---: | ---: | ---: | ---: |
| LibriSpeech test-clean (40 speakers) + ch.153 (4) | 44 | 928 | 3.23 | 2.22 | 29 |
| LibriSpeech test-other (33 speakers) | 33 | 661 | 3.78 | 1.48 | 20 |
| short utterances (<= 3 words) | 8 | 20 | 15.00 | 5.34 | 6 |
| FLEURS en (1) | 1 | 19 | 5.26 | 1.01 | 0 |
| numbers (LibriSpeech, tagged) | 26 | 783 | 4.09 | 3.75 | 16 |
| synthetic code terms (espeak-ng, 2 voices) | 40 | 322 | 27.64 | 20.36 | 8 |
| synthetic short (repo fixtures) | 4 | 31 | 0.00 | 0.00 | 4 |
| seam rows, each alone | 48 | 1163 | 2.67 | 1.43 | 32 |

Synthetic term recall: 59 of 80 terms spelled exactly (normalized).

## Speech in white noise (10 test-clean clips)

| Set | Clips | Words | WER % | CER % | Exact clips |
| --- | ---: | ---: | ---: | ---: | ---: |
| SNR 10 dB | 0 | 0 | 0.00 | 0.00 | 0 |
| SNR 0 dB | 0 | 0 | 0.00 | 0.00 | 0 |

## No speech (3 s held; 7 signals x 3 seeds)

Through the controller: 0 dictations, 0 reached the model, 0 produced text. Direct model calls without admission: 0 of 0 produced text. 

## Release offsets (pressed 300 ms before speech)

| Release vs end of speech | Clips | Last word exact | % | Last word dropped | No speech |
| ---: | ---: | ---: | ---: | ---: | ---: |
| -300 ms | 0 | 0 | 0.00 | 0 | 0 |
| -200 ms | 0 | 0 | 0.00 | 0 | 0 |
| -100 ms | 0 | 0 | 0.00 | 0 | 0 |
| 0 ms | 0 | 0 | 0.00 | 0 | 0 |
| 100 ms | 0 | 0 | 0.00 | 0 | 0 |
| 200 ms | 0 | 0 | 0.00 | 0 | 0 |

## Late presses (released at the end of speech)

| Press vs start of speech | Clips | First word exact | % | First word dropped | No speech |
| ---: | ---: | ---: | ---: | ---: | ---: |
| +0 ms | 0 | 0 | 0.00 | 0 | 0 |
| +100 ms | 0 | 0 | 0.00 | 0 | 0 |
| +200 ms | 0 | 0 | 0.00 | 0 | 0 |
| +300 ms | 0 | 0 | 0.00 | 0 | 0 |
| +450 ms | 0 | 0 | 0.00 | 0 | 0 |
| +600 ms | 0 | 0 | 0.00 | 0 | 0 |

## Seams (independent segments, not full-context recognition)

| Dictation | Audio s | Segments | WER % joined | WER % pause-split | Errors near seams | Pause-split near seams |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |

Seams: 0; errors within 3 reference words of a seam beyond the pause-split result: 0 (deletions near seams 0, insertions 0).

## Timings (ms; p99 is nearest-rank, so with fewer than 100 values it is the largest)

| Measure | n | p50 | p99 |
| --- | ---: | ---: | ---: |
| audio length per dictation | 201 | 5750.00 | 25990.00 |
| stop to final text (after the tail), all clips | 201 | 185.99 | 641.66 |
|   clips under 5 s | 90 | 146.00 | 368.38 |
|   clips 5-10 s | 58 | 171.82 | 466.43 |
|   clips 10-20.47 s | 53 | 252.16 | 1048.62 |
| engine call (frontend + encoder + decoder + text) | 201 | 78.77 | 356.72 |
|   CPU frontend and text | 201 | 31.98 | 234.07 |
|   CUDA encoder | 201 | 44.85 | 157.75 |
|   CUDA TDT decoder | 201 | 3.44 | 17.57 |
| stop to final text, 30-58 s dictations (2-3 calls) | 0 | 0.00 | 0.00 |
| model open, first in a new process | 3 | 19070.62 | 26841.16 |
| model open, again in the same process | 6 | 16481.20 | 27480.08 |
|   pkrt_open (Coax: layout, vocabulary, and the single-thread SHA-256 when not checked in pieces) | 9 | 14974.29 | 23512.74 |
|   the rest (piece hashing on threads, weight copy, CUDA start) | 9 | 1050.30 | 4514.69 |
|   NVRTC compile | 9 | 46.88 | 165.07 |
|   weight upload | 9 | 1138.52 | 5979.21 |
| model close | 9 | 437.97 | 1065.30 |
| voice host: turn end to transcript (4 fixtures x 10) | 0 | 0.00 | 0.00 |

## Gates

```
FAIL  WER test-clean speakers + ch.153 (%): 3.23 <= 3.00
pass  WER test-other speakers (%): 3.78 <= 8.00
FAIL  stop to final text p99, clips <= 20.47 s (ms): 641.66 <= 400.00
FAIL  model open p50, new process (ms): 19070.62 <= 12000.00
```
