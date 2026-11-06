# Optional browser pack digest gate, version 1

This internal browser/Wasm effect ABI is opt-in for the legacy pack loader.
Contract-v1 host consumers retain their separate Coax pack-reactor path. Existing realms that do not
export `coaxial_pack_gate_version` retain their established loading path.
It does not change the VPK1 format or a network protocol.

Before interpreting or uploading a received pack, the host computes SHA-256
with Web Crypto, then transports its 32 raw digest bytes and byte count to
the realm. Coax decides admission. This avoids copying large immutable art
packs through the realm's frame arena merely to check their identity.

The scalar exports are:

- `coaxial_pack_gate_version() -> u32`: exactly `1` for this ABI; another
  version refuses the pack.
- `coaxial_pack_digest(state) -> u32`: destination address of the realm's
  digest buffer.
- `coaxial_pack_digest_capacity() -> u32`: exactly 32 bytes. The host checks
  the complete destination against current Wasm memory before copying.
- `coaxial_pack_admit(state, pack_bytes:u32) -> u32`: Coax checks the received
  digest and length. Exactly `1` permits unpack/upload; any other result
  refuses every part. No GPU resource from that pack has been made yet.

Hash computation is a platform effect. Expected identities, byte limits and
acceptance stay in Coax. A realm exposing version 1 supplies every export.
Async digest completion checks that the original engine still owns the
request before writing its destination. The typed renderer's existing owner
check also applies. GPU recovery fetches and rechecks the immutable pack;
legacy pack rows clear their old-device buffers/textures before shader
binding can inspect them.

First consumer: Valley Lab's isolated Canyon Pair. Its authority module
`verse/valley/canyon-pack-policy.coax` pins the complete private source pack's
SHA-256 and byte count. Tests corrupt every digest lane and both exact length
boundaries. The real browser journey edits a byte inside the vertex payload,
preserving the complete pack length, and observes Coax refusal before upload.
It also checks exact rendered pixels and realm state across device loss.
