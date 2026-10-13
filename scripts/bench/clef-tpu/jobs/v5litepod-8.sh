# CLEFJOB v5e-8 plan. Concurrent pinned sub-slices crash together on v5e when one fails
# (SLICE_FAILURE_INIT_ERROR), so run one engine at a time.
cd ~
./pin.sh 0,1,2,3 2,2,1 8476 env MODEL=Cloudflare/clef-flash TP=4 LENS="1024 4096 16384" BATCHES="1 8 16" bash prefill.sh
MODEL=Cloudflare/clef TP=8 QUANT=text_int8_w8a8 LENS="1024 4096 16384" BATCHES="1 8 16" bash prefill.sh
MODEL=Cloudflare/clef-flash TP=8 QUANT=text_int8_w8a8 LENS="1024 4096 16384" BATCHES="1 8 16" bash prefill.sh
