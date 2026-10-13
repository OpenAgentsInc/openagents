# CLEFJOB v6e-4 plan. Spot v6e-4 lives 8-10 min here: short points first, then sweeps.
cd ~
Q='LENS=4096 BATCHES=8 REPEATS=2 TOKPAD=8192 BSPAD=8'
env $Q MODEL=Cloudflare/clef TP=4 QUANT=text_int8_w8a8 TAG=clef-tp4-int8w8a8-quick bash prefill.sh
env $Q MODEL=Cloudflare/clef TP=4 QUANT=text_int8 TAG=clef-tp4-int8wo-quick bash prefill.sh
env LENS="1024 16384" BATCHES="1 8" REPEATS=2 TOKPAD="1024 2048 4096 8192" BSPAD=8 MODEL=Cloudflare/clef TP=4 TAG=clef-tp4-bf16-short bash prefill.sh
MODEL=Cloudflare/clef TP=4 bash prefill.sh
