# Psionic token block verification, 2026-10-29

`qwen-tokens.coax` gathers token embeddings, executes a bound hybrid block,
normalizes its last hidden state and projects the complete vocabulary in
bounded chunks before option readout. The independent fixtures exercise a
recurrent prefix and cached attention suffix; a native fixture crosses the
32,768-row chunk boundary and checks refusal and alias behavior.

On the development Mac using coax0 0.16.0, 147 Coax checks passed in debug and release; all
3,542 mutation candidates were killed. Native debug and release checks passed
both normally and with ASan/UBSan. Native release checks also passed on
CoderOS. These tests use bounded synthetic models; the separately documented
original-weight work supplies real-model evidence.
