# CLEFJOB v6e-1 plan
cd ~
MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8_w8a8 bash prefill.sh
BENCHES="matmul attn" bash kernels.sh
MODEL=Cloudflare/clef-flash TP=1 LOAD=auto LENS="1024 4096" BATCHES="1 8" bash prefill.sh
