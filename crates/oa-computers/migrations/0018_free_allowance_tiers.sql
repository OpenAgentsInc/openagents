-- Free cloud allowance tiers (#11994). An account made with only a Nostr
-- key on an attested phone install gets the smaller attested allowance; it
-- moves to the full allowance once it connects an account or pays. A tier
-- only moves up: enrolling an attested account as full upgrades it, and
-- enrolling a full account as attested changes nothing.
ALTER TABLE computers.free_allowance
    ADD COLUMN attested_boot_cpu_ms bigint NOT NULL DEFAULT 40000000
        CHECK(attested_boot_cpu_ms >= 0);

ALTER TABLE computers.free_accounts
    ADD COLUMN tier text NOT NULL DEFAULT 'full' CHECK(tier IN ('full', 'attested'));
