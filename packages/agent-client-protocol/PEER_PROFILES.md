# Trusted peer profiles and registry admission (ACP-9 #8896)

`@openagentsinc/agent-client-protocol/profiles` is the declarative
peer-profile contract, trusted registry, and fail-closed admission path for
foreign Agent Client Protocol agents. Grok CLI (`grok agent stdio`) and Cursor
Agent CLI (`agent acp`) are the two reference profiles; both use the same
validated contract and keep their vendor extensions in separate modules
(`./extensions/grok`, `./extensions/cursor`).

## The contract in one paragraph

A profile is data, never code. It pins provider/profile identity and revision,
provenance, supported/experimental/denied version ranges, the exact executable
discovery strategy and argument array, allowlisted environment keys and secret
references (names only, never values), the expected executable and initialize
identity, auth-method policy and interaction mappings, stable capability
expectations and known deviations, mode/model configuration ids, a versioned
vendor-extension allowlist, session ownership/restore/cancel/shutdown policy,
required conformance evidence and its freshness, platform support,
installation guidance, and redaction additions. Parsing
(`parseAcpTrustedPeerProfile`) rejects unknown keys, unknown executable
strategies, shell-like strings, path traversal, unbounded values, undeclared
environment keys, unpinned or invalid version ranges, and extension namespace
collisions (including collisions with the pinned stable/unstable method
manifests).

## Trust boundaries and threat model

### Boundary 1 — official registry snapshot → discovery metadata

Threats: a compromised or drifted registry serving hostile launch commands,
installer URLs, oversized payloads, duplicate/renamed providers, or entries
crafted to be interpolated into a shell.

Mitigations: `ingestOfficialAcpRegistrySnapshot` is bounded (256 KiB / 128
entries / bounded field lengths), validated against a local schema,
digest-verified when a pin is supplied, and deterministic (sorted entries,
deterministic duplicate resolution with dropped ids recorded). The projected
`AcpRegistryDiscoveryEntry` type structurally contains **no executable, argv,
environment, installer, or extension fields**, so a snapshot can never confer
execution or authority. The snapshot digest is carried into diagnostics and
registry-derived profile provenance.

### Boundary 2 — profile document → trusted registry

Threats: a malicious or sloppy profile smuggling shell metacharacters or
traversal into argv, undeclared environment keys (`PATH`, `LD_PRELOAD`,
`NODE_OPTIONS` style injection), wildcard/unpinned versions, an extension
method that shadows a stable protocol method, or one provider claiming another
provider's extension namespace.

Mitigations: the strict parser (fail-closed, typed rejection reasons, bounded
everything, conservative argv charset, traversal rejection, environment-key
allowlist closure, pinned numeric version ranges with denied-range conflict
checks, extension namespace validation against the pinned method manifests).
`createAcpTrustedPeerProfileRegistry` additionally rejects duplicate profile
ids and cross-profile extension ownership collisions. Registered profiles are
deep-frozen clones.

### Boundary 3 — install/executable → launch

Threats: PATH shadowing, symlink retargeting, package-manager shim swaps,
post-install replacement, version drift, an impostor binary answering the
probe, and caller-supplied argv/env overrides.

Mitigations: launch plans exist only via `resolveAcpTrustedLaunchPlan` and are
copied exclusively from the registered profile; admission refuses any
`requestedLaunchOverride` outright (`caller_launch_override_rejected`).
`evaluateAcpExecutableTrust` verifies platform, requested-executable and
resolved-basename identity, a pinned `x.y.z` version outside every denied
range, and — once an identity pin exists — refuses any realpath or sha-256
change as `path_replacement` before a session starts.
`buildAdmittedLaunchEnvironment` filters the environment to the profile
allowlist (dropping undeclared keys) and fails closed on missing required
secrets. Installation is guidance-only in this layer: nothing here executes an
installer, and no registry/profile string ever reaches a shell.

### Boundary 4 — admission → capability authority

Threats: capability lies at initialize, undeclared vendor extensions, wire
version treated as a support claim, and unknown peers acquiring filesystem,
terminal, auto-approval, or network authority.

Mitigations: `deriveAcpSupportState` derives supported/experimental/
incompatible from profile version ranges **plus** fresh, digest-bound
conformance evidence — never from a provider name or wire version alone; with
no pinned live evidence the reference profiles admit at most `experimental`.
Observed capabilities and extension methods outside the profile are
quarantined, not granted. Grants keep `permissionAutoApproval` structurally
`false` (the authority broker of #8891 owns interactive permission decisions)
and enable fs/terminal/network only for `supported` peers whose profile
declares them supported. Unknown peers run only through
`admitUnknownAcpPeerExperimental` with the explicit acknowledgement literal,
every risky grant disabled, and strict bounds.

## Diagnostics and evidence

Admission decisions carry sanitized diagnostics: profile/provider ids,
revision, contract and schema release, support state, peer version, executable
basename plus sha-256, registry snapshot digest when provenance is
registry-derived, and evidence artifact refs — no filesystem paths, secrets,
or environment values.

Profile changes live under `packages/agent-client-protocol/src/profiles/**`.
Their required local or owned-runner gate runs this package's generated check,
typecheck, and tests together with the conformance package's artifact check,
typecheck, and tests. Repository policy forbids GitHub-hosted workflows; an
agent or owned runner invokes those package commands and records their evidence.
The pinned **live** compatibility matrix required to mark a peer `supported` is
owned by the release gate (#8897); its passing records enter admission as
`kind: "live"` evidence bound to the exact binary digest.

## Adding a peer

1. Author a declarative profile against `parseAcpTrustedPeerProfile` (new
   module under `src/profiles/`); put any vendor extensions in their own
   module under `src/extensions/`.
2. Register it in the trusted registry; hermetic tests must pass.
3. Ship fixture conformance evidence; the peer admits as `experimental`.
4. Land pinned live-matrix evidence (#8897) and a `supported` version range in
   the same revision bump; only then does admission derive `supported`.
