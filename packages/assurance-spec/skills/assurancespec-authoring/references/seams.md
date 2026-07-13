# Seam obligations

A seam is behavior that exists only when two real components are wired
together: renderer to host, client to API, API to storage, compiler to adapter,
or producer receipt to consumer projection.

Give each material seam its own obligation. Name both real sides, the boundary
contract, the environment tier, a wiring oracle, and a relationship-breaking
falsifier. Component tests with mocks on both sides do not satisfy this
obligation. They may support the components while the seam remains unobserved.

Keep seam failures distinct from infrastructure failures. A missing real side
is a typed gap, not a confirmed seam.
