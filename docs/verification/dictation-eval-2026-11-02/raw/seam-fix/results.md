## Quality (held 300 ms before speech to 200 ms after; GPU)

| Set | Clips | Words | WER % | CER % | Exact clips |
| --- | ---: | ---: | ---: | ---: | ---: |
| LibriSpeech test-clean (40 speakers) + ch.153 (4) | 44 | 928 | 1.94 | 0.60 | 30 |
| LibriSpeech test-other (33 speakers) | 33 | 661 | 3.78 | 1.48 | 20 |
| short utterances (<= 3 words) | 8 | 20 | 15.00 | 5.34 | 6 |
| FLEURS en (1) | 1 | 19 | 5.26 | 1.01 | 0 |
| numbers (LibriSpeech, tagged) | 26 | 783 | 1.28 | 0.46 | 18 |
| synthetic code terms (espeak-ng, 2 voices) | 40 | 322 | 27.64 | 20.36 | 8 |
| synthetic short (repo fixtures) | 4 | 31 | 0.00 | 0.00 | 4 |
| seam rows, each alone | 48 | 1163 | 1.81 | 0.57 | 33 |

Synthetic term recall: 59 of 80 terms spelled exactly (normalized).

## Speech in white noise (10 test-clean clips)

| Set | Clips | Words | WER % | CER % | Exact clips |
| --- | ---: | ---: | ---: | ---: | ---: |
| SNR 10 dB | 10 | 271 | 4.06 | 1.37 | 4 |
| SNR 0 dB | 10 | 271 | 9.59 | 4.77 | 1 |

## No speech (3 s held; 7 signals x 3 seeds)

Through the controller: 21 dictations, 12 reached the model, 1 produced text. Direct model calls without admission: 0 of 21 produced text. white-40dBFS#3: "Uh"; 

## Release offsets (pressed 300 ms before speech)

| Release vs end of speech | Clips | Last word exact | % | Last word dropped | No speech |
| ---: | ---: | ---: | ---: | ---: | ---: |
| -300 ms | 114 | 100 | 87.72 | 0 | 0 |
| -200 ms | 114 | 112 | 98.25 | 0 | 0 |
| -100 ms | 114 | 113 | 99.12 | 0 | 0 |
| 0 ms | 114 | 113 | 99.12 | 0 | 0 |
| 100 ms | 114 | 111 | 97.37 | 0 | 0 |
| 200 ms | 114 | 114 | 100.00 | 0 | 0 |

## Late presses (released at the end of speech)

| Press vs start of speech | Clips | First word exact | % | First word dropped | No speech |
| ---: | ---: | ---: | ---: | ---: | ---: |
| +0 ms | 118 | 116 | 98.31 | 0 | 0 |
| +100 ms | 118 | 116 | 98.31 | 0 | 0 |
| +200 ms | 118 | 117 | 99.15 | 0 | 0 |
| +300 ms | 118 | 115 | 97.46 | 1 | 0 |
| +450 ms | 118 | 112 | 94.92 | 4 | 1 |
| +600 ms | 118 | 76 | 64.41 | 30 | 3 |

Words lost inside the supported timing (release 0..+200 ms, press up to +300 ms):

```
changed release +100 ms 4507-16021-0000: "CHAPTER IRIGING"
changed release +0 ms s01-en-us: "Open the file voice.cox and run the text."
changed release +100 ms s01-en-us: "Open the file voice.cox and run the text."
changed release +100 ms s02-en-gb-scotland: "Sionic runs parakeet on the DPU."
DROPPED press +300 ms 7127-75947-0000: "Everyone could observe his agitation and prostration, a prostration which was, indeed, the more remarkable since people were not accustomed to see him, with his arms hanging listlessly by his side, his head bewildered, and his eyes with all their bright intelligence bedimmed."
changed press +300 ms 1284-1181-0000: "Oju examined this curious contrivance with wonder."
changed press +100 ms 260-123288-0014: "Han stirs nod."
changed press +0 ms s03-en-gb-scotland: "Cole C D submit after the draft is ready."
changed press +0 ms s16-en-us: "CUBA kernels run on the RTX for the AD."
changed press +200 ms s16-en-us: "CUBA kernels run on the RTX for the A B."
changed press +300 ms s16-en-us: "CUBA kernels run on the RTX for the A B."
changed press +100 ms s17-en-gb-scotland: "Kicks keeps a 450m preroll."
```

## Seams (independent segments, not full-context recognition)

| Dictation | Audio s | Segments | WER % joined | WER % pause-split | Errors near seams | Pause-split near seams | Errors beyond pause-split |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| seam-6930-75918 | 46 | 3 | 0.00 | 0.00 | 0 | 0 | 0 |
| seam-6930-76324 | 31 | 2 | 5.15 | 5.15 | 0 | 0 | 0 |
| seam-6930-81414 | 31 | 2 | 1.32 | 1.32 | 0 | 0 | 0 |
| seam-1320-122617 | 36 | 2 | 0.97 | 1.94 | 0 | 0 | 0 |
| seam-1320-122612 | 31 | 2 | 0.00 | 0.00 | 0 | 0 | 0 |
| seam-5639-40744 | 37 | 3 | 2.83 | 2.83 | 0 | 0 | 0 |

Seams: 8; errors within 3 reference words of a seam beyond the pause-split result: 0 (deletions near seams 0, insertions 0). Whole dictations: 0 word errors beyond pause-split in 6 dictations.

## Timings (ms; p99 is nearest-rank, so with fewer than 100 values it is the largest)

| Measure | n | p50 | p99 |
| --- | ---: | ---: | ---: |
| audio length per dictation | 201 | 5750.00 | 25990.00 |
| stop to final text (after the tail), all clips | 201 | 73.22 | 318.69 |
|   clips under 5 s | 90 | 58.89 | 318.69 |
|   clips 5-10 s | 58 | 79.85 | 172.39 |
|   clips 10-20.47 s | 53 | 131.55 | 356.55 |
| engine call (frontend + encoder + decoder + text) | 201 | 64.25 | 235.45 |
|   CPU frontend and text | 201 | 21.20 | 112.67 |
|   CUDA encoder | 201 | 41.06 | 138.84 |
|   CUDA TDT decoder | 201 | 2.85 | 11.23 |
| stop to final text, 30-58 s dictations (2-3 calls) | 6 | 365.31 | 588.03 |
| model open, first in a new process | 3 | 9043.12 | 9209.51 |
| model open, again in the same process | 6 | 8210.81 | 10364.98 |
|   pkrt_open (Coax: layout, vocabulary, and the single-thread SHA-256 when not checked in pieces) | 9 | 54.37 | 76.25 |
|   the rest (piece hashing on threads, weight copy, CUDA start) | 9 | 6138.67 | 7782.69 |
|   NVRTC compile | 9 | 129.99 | 193.27 |
|   weight upload | 9 | 2685.91 | 5459.85 |
| model close | 9 | 617.32 | 1016.56 |
| voice host: turn end to transcript (4 fixtures x 10) | 40 | 125.00 | 342.00 |

Voice host transcripts exact: 40 of 40.

## Gates

```
pass  WER test-clean speakers + ch.153 (%): 1.94 <= 4.00
pass  WER test-other speakers (%): 3.78 <= 5.00
pass  WER at 10 dB SNR (%): 4.06 <= 5.00
pass  no-speech dictations that produce text: 1.00 <= 1.00
pass  last word exact, released 0..+200 ms (%): 98.83 >= 97.00
pass  last words dropped, released 0..+200 ms: 0.00 <= 0.00
pass  first word exact, pressed 0..+300 ms late (%): 98.31 >= 97.00
pass  first words dropped, pressed 0..+300 ms late: 1.00 <= 1.00
pass  seam errors beyond pause-split, all seams: 0.00 <= 0.00
pass  long dictations: word errors beyond pause-split: 0.00 <= 0.00
pass  stop to final text p99, clips <= 20.47 s (ms): 318.69 <= 600.00
pass  stop to final text p99, 30-58 s (ms): 588.03 <= 1000.00
pass  model open p50, new process (ms): 9043.12 <= 12000.00
pass  voice host turn end to transcript p99 (ms): 342.00 <= 400.00
```
