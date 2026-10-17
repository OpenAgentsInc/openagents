-- The free cloud allowance (#11981). The front door enrolls an account here
-- when it starts Our cloud for a signed-in account that has no invitation.
-- An enrolled account with no row of its own in usage_limits is held to the
-- single free_allowance row; an account's own usage_limits row always
-- replaces it. Without a free_allowance row, enrolled accounts are refused.
CREATE TABLE computers.free_allowance (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    computer_count bigint NOT NULL CHECK(computer_count >= 0),
    boot_cpu_ms bigint NOT NULL CHECK(boot_cpu_ms >= 0),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE computers.free_accounts (
    owner text PRIMARY KEY CHECK(length(owner) BETWEEN 1 AND 256),
    enrolled_at timestamptz NOT NULL DEFAULT now()
);
