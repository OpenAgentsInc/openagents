# Access

Who may make realms. The switches: INVITE ONLY (when on, only approved
creators can make a realm) and NEW LOGIN. NEW INVITE makes an invite link
for the picked uses (1, 10 or 100) and days (7, 30 or 90); the link shows
once, with COPY LINK. Live invites have REVOKE, waiting requests have
APPROVE and DECLINE, and GRANDFATHER makes everyone who already owns a
realm a creator. The creators are listed last.

Admins only, with the same login as [Analytics](/admin/analytics). Anyone
else sees nothing. The rules are in
[docs/auth/2026-10-31-realm-invites-and-auth.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/auth/2026-10-31-realm-invites-and-auth.md)
and `coax/web/api/migrations/0090_realm_invites.sql`.
