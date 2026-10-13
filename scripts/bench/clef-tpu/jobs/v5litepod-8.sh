: CLEFJOB v5e-8 plan (re-acquired after the guard bug)
cd ~
MODEL=Cloudflare/clef TP=8 bash prefill.sh
(./pin.sh 0 1,1,1 8476 env MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8_w8a8 MEMFRAC=0.8 MAXRUN=32 bash prefill.sh) &
(./pin.sh 1 1,1,1 8477 env MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8 MEMFRAC=0.8 MAXRUN=32 bash prefill.sh) &
(./pin.sh 4,5,6,7 2,2,1 8478 env MODEL=Cloudflare/clef-flash TP=4 bash prefill.sh) &
wait
