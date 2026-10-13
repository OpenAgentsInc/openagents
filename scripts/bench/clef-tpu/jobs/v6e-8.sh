: CLEFJOB v6e-8 standard plan: 5 concurrent pinned jobs
cd ~
(./pin.sh 0,1,2,3 2,2,1 8476 env MODEL=Cloudflare/clef TP=4 bash prefill.sh; ./pin.sh 0,1,2,3 2,2,1 8476 env MODEL=Cloudflare/clef TP=4 QUANT=text_int8_w8a8 bash prefill.sh) &
(./pin.sh 4 1,1,1 8480 env MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8_w8a8 bash prefill.sh; ./pin.sh 4 1,1,1 8480 env MODEL=Cloudflare/clef-flash TP=1 CHUNK=16384 TOKPAD="1024 2048 4096 8192 16384" bash prefill.sh) &
(./pin.sh 5 1,1,1 8481 env MODEL=Cloudflare/clef-flash TP=1 QUANT=text_int8 bash prefill.sh; ./pin.sh 5 1,1,1 8481 env MODEL=Cloudflare/clef-flash TP=1 LOAD=auto LENS="1024 4096" BATCHES="1 8" bash prefill.sh) &
(./pin.sh 6 1,1,1 8482 env BENCHES="matmul attn gdn" bash kernels.sh; ./pin.sh 6 1,1,1 8482 bash -c "source ~/venv/bin/activate; export PYTHONPATH=~/sglang-jax/python; GATE=long python -u bench_gdn_state.py > ~/out/log-gdn-state-long.txt 2>&1") &
(./pin.sh 7 1,1,1 8483 env MODEL=Cloudflare/clef TP=1 QUANT=text_int8_w8a8 MEMFRAC=0.92 LENS="1024 4096 16384" bash prefill.sh) &
wait
(./pin.sh 0,1,2,3,4,5,6,7 2,4,1 8476 env MODEL=Cloudflare/clef TP=8 bash prefill.sh)
