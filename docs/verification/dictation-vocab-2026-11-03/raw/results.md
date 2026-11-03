## Vocabulary rescoring: without and with the vocabulary

| Set | Clips | Words | WER % off | WER % on | CER % off | CER % on | Terms | Right off | Right on | Clips changed | Words changed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| LibriSpeech test-clean + test-other speakers (no terms) | 73 | 1543 | 2.79 | 2.79 | 0.99 | 0.99 | 0 | 0 | 0 | 0 | 0 |
| held-out distractors (synthetic, no terms) | 40 | 280 | 7.50 | 7.50 | 3.76 | 3.76 | 0 | 0 | 0 | 0 | 0 |
| #12665 code-term clips (synthetic) | 40 | 322 | 27.64 | 26.09 | 20.36 | 19.78 | 80 | 59 | 63 | 5 | 6 |
| held-out term clips (synthetic) | 40 | 248 | 20.16 | 15.32 | 7.95 | 6.43 | 42 | 13 | 23 | 10 | 12 |

False replacements (term-free clips the vocabulary changed): 0 clips, 0 words. Records whose text before rescoring differs from the run without a vocabulary: 0 of 193.

| Clip | Without | With |
| --- | --- | --- |
| s06-en-gb-scotland | Push the commit to main and replace on origin main. | Push the commit to main and rebase on origin main. |
| s09-en-gb-scotland | Christopher asked the eron to revill request 12665. | Christopher asked Zeron to revill request 12665. |
| s10-en-us | OpenAgen's dictate prints the final text. | OpenAgents dictate prints the final text. |
| s15-en-gb-scotland | Noster events are signed with Schnorr signatures. | Nostr events are signed with Schnorr signatures. |
| s19-en-us | Kubernetes, JSON and WebDPU are on the list. | Kubernetes, JSON and WebGPU are on the list. |
| v01-en-us | The Cooks book explains every chapter. | The Coax book explains every chapter. |
| v03-en-us | Arakeet listens on the laptop. | Parakeet listens on the laptop. |
| v05-en-us | Xiron opened a pull request this morning. | Zeron opened a pull request this morning. |
| v05-en-gb-scotland | Zero opened a pull request this morning. | Zeron opened a pull request this morning. |
| v09-en-gb-scotland | Pass the gase on before you store it. | Pass the JSON before you store it. |
| v12-en-us | Draw it with WebDPU in the browser. | Draw it with WebGPU in the browser. |
| v12-en-gb-scotland | Draw it with Web DPU in the browser. | Draw it with WebGPU in the browser. |
| v14-en-us | OpenAGENS is the name of the company. | OpenAgents is the name of the company. |
| v17-en-us | Sidonic and Coke ship together. | Sidonic and Coax ship together. |
| v17-en-gb-scotland | Sionic and Cloak's ship together. | Sionic and Coax ship together. |

| Stop to final text (ms) | n | p50 | p99 |
| --- | ---: | ---: | ---: |
| without a vocabulary | 193 | 114.34 | 532.60 |
| with the vocabulary | 193 | 147.67 | 746.43 |

## Gates

```
pass  term-free WER rise with the vocabulary (points): 0.00 <= 0.30
pass  text before rescoring differs from the run without: 0.00 <= 0.00
```
