-- Public receipt projection for reliable-tip ladder pay-ins.
--
-- Issue #4747: Artanis responder tips already land on the pay_ins
-- ledger, but the recipient/public surfaces need a stable public
-- receipt ref that resolves without exposing idempotency keys, raw
-- payment material, wallet refs, or provider payloads.

ALTER TABLE pay_ins ADD COLUMN public_receipt_ref TEXT;

UPDATE pay_ins
   SET public_receipt_ref =
       'receipt.forum.tip_ladder.artanis_responder.' ||
       replace(
         substr(idempotency_key, length('artanis-responder-tip:') + 1),
         ':credited_fallback',
         ''
       )
 WHERE pay_in_type = 'tip'
   AND public_receipt_ref IS NULL
   AND idempotency_key LIKE 'artanis-responder-tip:%';

CREATE INDEX IF NOT EXISTS idx_pay_ins_public_receipt_ref
  ON pay_ins(public_receipt_ref)
  WHERE public_receipt_ref IS NOT NULL;

ALTER TABLE artanis_responder_actions ADD COLUMN tip_receipt_ref TEXT;
ALTER TABLE artanis_responder_actions ADD COLUMN tip_pay_in_id TEXT;
ALTER TABLE artanis_responder_actions ADD COLUMN tip_ladder_rung TEXT;
ALTER TABLE artanis_responder_actions ADD COLUMN tip_ladder_reason TEXT;

UPDATE artanis_responder_actions
   SET tip_receipt_ref =
       'receipt.forum.tip_ladder.artanis_responder.' || topic_id
 WHERE state = 'tipped'
   AND tip_receipt_ref IS NULL;

UPDATE artanis_responder_actions
   SET tip_pay_in_id = (
         SELECT p.id
           FROM pay_ins p
          WHERE p.public_receipt_ref = artanis_responder_actions.tip_receipt_ref
            AND p.state = 'paid'
          ORDER BY p.created_at DESC, p.id DESC
          LIMIT 1
       ),
       tip_ladder_rung = (
         SELECT p.rung
           FROM pay_ins p
          WHERE p.public_receipt_ref = artanis_responder_actions.tip_receipt_ref
            AND p.state = 'paid'
          ORDER BY p.created_at DESC, p.id DESC
          LIMIT 1
       )
 WHERE state = 'tipped'
   AND tip_receipt_ref IS NOT NULL
   AND tip_pay_in_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_artanis_responder_actions_tip_receipt
  ON artanis_responder_actions(tip_receipt_ref)
  WHERE tip_receipt_ref IS NOT NULL;
