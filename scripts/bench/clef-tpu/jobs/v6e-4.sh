: CLEFJOB v6e-4 plan
cd ~
MODEL=Cloudflare/clef TP=4 bash prefill.sh
MODEL=Cloudflare/clef TP=4 QUANT=text_int8_w8a8 bash prefill.sh
MODEL=Cloudflare/clef-flash TP=4 bash prefill.sh
