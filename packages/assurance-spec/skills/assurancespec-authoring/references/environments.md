# Environment-bound evidence

Bind each obligation to explicit Environment Profile IDs. An environment
defines the execution tier and capabilities needed to interpret evidence:
fixture, local deterministic runtime, browser, packaged desktop, staging, or
production-like infrastructure.

Evidence never silently upgrades tiers. A fixture observation remains fixture
evidence even if the same command might run elsewhere. Missing profiles or
capabilities produce `environment_profile_missing`,
`environment_profiles_need_design`, or another typed gap; they never produce a
green skip.

Record the exact profile digest when the profile format supplies one. If the
profile changes, the previous evidence remains attached to its original
environment identity and freshness must be assessed separately.
