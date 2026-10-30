# Coax API bridge: work in progress

`coax_probe.coax` exercises the shared Nostr JSON parser on fixed public
fixtures. It does not verify signatures, authorize requests, or replace the
service's existing NIP-98 implementation. It is not yet linked into the API.

On 2026-10-29 the module passed 21 Coax checks. An experimental service
integration passed 923 local service checks against throwaway Postgres,
including existing NIP-98 and phone sign-in/link/replay cases, and correctly
returned 503 for a deliberately failing probe. That experimental integration
is not shipped. A Linux cloud image build from source exceeded its 900-second
limit during the full compiler bootstrap (Cloud Build
`565a0656-481f-4949-bf92-b87a2ae373c3`); no image was deployed.

The next step is a reproducible Linux build that completes within the build
limit without weakening or repeating established compiler checks unnecessarily,
then the actual linked service integration and deployment. Preserve the
existing authentication behavior and the independently owned phone endpoints.
