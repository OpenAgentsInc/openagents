# CLEFJOB v6e-8 plan. Spot v6e-8 lives 8-14 min here, so short points first (one compile bucket each,
# shared compile cache), then the long sweeps if the slice survives.
cd ~
Q='LENS=4096 BATCHES=8 REPEATS=2 TOKPAD=8192 BSPAD=8'
(./pin.sh 0,1,2,3 2,2,1 8476 env $Q MODEL=Cloudflare/clef TP=4 QUANT=text_int8_w8a8 TAG=clef-tp4-int8w8a8-quick bash prefill.sh) &
(./pin.sh 4 1,1,1 8480 env $Q MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8_w8a8 TAG=flash-tp1-int8w8a8-quick bash prefill.sh) &
(./pin.sh 5 1,1,1 8481 env $Q MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8 TAG=flash-tp1-int8wo-quick bash prefill.sh) &
(./pin.sh 6 1,1,1 8482 env $Q MODEL=Cloudflare/clef-flash TP=1 CHUNK=16384 TOKPAD=16384 LENS=16384 BATCHES=2 TAG=flash-tp1-bf16-chunk16k-quick bash prefill.sh) &
(./pin.sh 7 1,1,1 8483 env MODEL=Qwen/Qwen3.5-9B TP=1 LOAD=auto LENS=4096 BATCHES=8 REPEATS=2 TOKPAD=8192 BSPAD=8 TAG=qwen9b-tp1-bf16-realweights bash prefill.sh) &
wait
(./pin.sh 0,1,2,3 2,2,1 8476 env MODEL=Cloudflare/clef TP=4 bash prefill.sh) &
(./pin.sh 4 1,1,1 8480 env MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8_w8a8 bash prefill.sh) &
(./pin.sh 5 1,1,1 8481 env MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8 bash prefill.sh) &
(./pin.sh 6 1,1,1 8482 env BENCHES="matmul attn" bash kernels.sh) &
(./pin.sh 7 1,1,1 8483 env MODEL=Cloudflare/clef TP=1 QUANT=text_int8_w8a8 MEMFRAC=0.8 LENS=4096 BATCHES="1 4" bash prefill.sh) &
wait
