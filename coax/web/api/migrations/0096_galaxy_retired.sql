-- /verse/galaxy is retired (it redirects to /realms): the galaxy leaves the
-- analytics contract (coax/web/analytics-contract.json), so the admin health
-- card stops waiting for its page-view. Its past events stay.
DELETE FROM analytics.contract WHERE surface = 'galaxy';
