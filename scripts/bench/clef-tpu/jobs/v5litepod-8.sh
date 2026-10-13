# CLEFJOB v5e-8 plan, re-acquired after the guard bug. int8 quick points first.
cd ~
Q='LENS=4096 BATCHES=8 REPEATS=2 TOKPAD=8192 BSPAD=8 MEMFRAC=0.8 MAXRUN=32'
(./pin.sh 0 1,1,1 8476 env $Q MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8_w8a8 TAG=flash-tp1-int8w8a8-quick bash prefill.sh) &
(./pin.sh 1 1,1,1 8477 env $Q MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8 TAG=flash-tp1-int8wo-quick bash prefill.sh) &
(./pin.sh 4,5,6,7 2,2,1 8478 env MODEL=Cloudflare/clef-flash TP=4 bash prefill.sh) &
(./pin.sh 2,3 1,2,1 8479 env MODEL=Cloudflare/clef-flash TP=2 bash prefill.sh) &
wait
(./pin.sh 0 1,1,1 8476 env MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8_w8a8 MEMFRAC=0.8 MAXRUN=32 bash prefill.sh) &
(./pin.sh 4,5,6,7 2,2,1 8478 env MODEL=Cloudflare/clef TP=4 QUANT=text_int8_w8a8 bash prefill.sh) &
wait
MODEL=Cloudflare/clef TP=8 QUANT=text_int8_w8a8 bash prefill.sh
