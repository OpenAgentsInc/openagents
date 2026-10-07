-- Refunds consume original merchant funding. Payout reservations retain their
-- original amounts even when later evidence reverses commission eligibility.
DROP VIEW IF EXISTS payable_share;
DROP VIEW IF EXISTS commission_balance;
CREATE VIEW commission_balance AS
WITH original AS (
 SELECT o.*,
 COALESCE((SELECT SUM(r.amount_msat) FROM commission_reversal r WHERE r.admission=o.id),0) AS refunds,
 COALESCE((SELECT SUM(n.amount_msat) FROM native_payout_claim n JOIN payout p ON p.id=n.payout WHERE n.settlement=o.settlement AND n.party=o.party AND n.role='commission' AND p.state!='failed'),0) AS protected,
 COALESCE((SELECT SUM(COALESCE(n.amount_msat,s.amount_msat)) FROM payout_item i JOIN payout p ON p.id=i.payout JOIN share s ON s.settlement=i.settlement AND s.party=i.party AND s.role=i.role LEFT JOIN native_payout_claim n ON n.payout=i.payout AND n.settlement=i.settlement AND n.party=i.party AND n.role=i.role WHERE i.settlement=o.settlement AND i.party='openagents' AND i.role='openagents' AND p.state!='failed'),0) AS oa_protected,
 (SELECT amount_msat FROM share WHERE settlement=o.settlement AND party='openagents' AND role='openagents') AS original_oa,
 COALESCE((SELECT SUM(amount_msat) FROM bonus_funding WHERE source_settlement=o.settlement),0) AS bonus_funding,
 COALESCE((SELECT SUM(reduced_msat) FROM payable_adjustment WHERE settlement=o.settlement AND party='openagents' AND role='openagents'),0) AS other_reductions
 FROM commission_obligation o
), remaining AS (
 SELECT *,MAX(0,earned_msat-reversed_msat) AS net FROM original
), encumbered AS (
 SELECT *,CASE WHEN state='earned' THEN MAX(net,protected) WHEN state='held' THEN MAX(0,base_msat-refunds) ELSE 0 END AS encumbered,
 CASE WHEN state='earned' THEN MAX(0,net-protected)%1000 ELSE 0 END AS remainder,
 MAX(0,protected-net) AS payee_loss FROM remaining
)
SELECT *,MAX(0,refunds+encumbered+oa_protected+bonus_funding+other_reductions-original_oa) AS funding_loss FROM encumbered;

CREATE VIEW payable_share AS
SELECT s.settlement,s.party,s.role,MAX(0,s.amount_msat
 - CASE WHEN s.party='openagents' AND s.role='openagents' THEN
 COALESCE((SELECT SUM(f.amount_msat) FROM bonus_funding f WHERE f.source_settlement=s.settlement),0)
 +COALESCE((SELECT SUM(c.encumbered+c.refunds) FROM commission_balance c WHERE c.settlement=s.settlement),0)
 ELSE 0 END
 - COALESCE((SELECT SUM(a.reduced_msat) FROM payable_adjustment a WHERE a.settlement=s.settlement AND a.party=s.party AND a.role=s.role),0)) AS amount_msat FROM share s
UNION ALL SELECT settlement,party,kind,amount_msat FROM bonus WHERE kind='first_paid_call'
UNION ALL SELECT c.settlement,c.party,'commission',MAX((c.net/1000)*1000,c.protected)
 FROM commission_balance c WHERE c.state='earned';
