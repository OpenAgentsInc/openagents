-- Retire the standalone Khala MPP/x402 chat endpoint replay caches (#8387).
--
-- The `/mpp/v1/chat/completions` route and its card/SPT + Lightning replay
-- writers were removed in favor of the keyed Khala Code launch path. Khala Code
-- paid-plan Lightning purchases use their own payment-intent ledger and do not
-- read either cache.

DROP TABLE IF EXISTS mpp_spt_replay;
DROP TABLE IF EXISTS mpp_lightning_replay;
