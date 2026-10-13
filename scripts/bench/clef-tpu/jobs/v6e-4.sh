# CLEFJOB v6e-4 plan. Spot v6e-4 lives 3-10 min here: short points first, then sweeps.
cd ~
Q='LENS=4096 BATCHES=8 REPEATS=2 TOKPAD=8192 BSPAD=8'
(./pin.sh 0,1 1,2,1 8476 env $Q MODEL=Cloudflare/clef TP=2 QUANT=text_int8_w8a8 MEMFRAC=0.9 TAG=clef-tp2-int8w8a8-quick bash prefill.sh) &
(./pin.sh 2 1,1,1 8478 env $Q MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8_w8a8 LENS=1024 BATCHES=32 TAG=flash-tp1-int8w8a8-1kx32-quick bash prefill.sh) &
(./pin.sh 3 1,1,1 8479 env $Q MODEL=Cloudflare/clef-flash TP=1 CHUNK=16384 TOKPAD=16384 LENS=16384 BATCHES=2 TAG=flash-tp1-bf16-chunk16k-quick bash prefill.sh) &
wait
env LENS="1024 4096 16384" BATCHES="1 8 16" REPEATS=2 TOKPAD="1024 2048 4096 8192" BSPAD=8 MODEL=Cloudflare/clef TP=4 QUANT=text_int8_w8a8 TAG=clef-tp4-int8w8a8-short bash prefill.sh
MODEL=Cloudflare/clef TP=4 bash prefill.sh
