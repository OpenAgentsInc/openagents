# CAD Compact IR Parity

Issue coverage: `VCAD-PARITY-086`

## Reference

- vcad compact IR implementation: `~/code/vcad/crates/vcad-ir/src/compact.rs`
- vcad compact IR spec: `~/code/vcad/docs/features/compact-ir.md`
- OpenAgents fixture: `crates/cad/parity/fixtures/compact_ir_vcad_reference.json`

## Manifest

Generated manifest path:

- `crates/cad/parity/compact_ir_parity_manifest.json`

Generate/check:

```bash
cargo run -p openagents-cad --bin parity-compact-ir
cargo run -p openagents-cad --bin parity-compact-ir -- --check
```

## CI Lane

```bash
scripts/cad/parity-compact-ir-ci.sh
```

Lane asserts:

- opcode coverage matches vcad fixture contract
- parse error diagnostics include invalid-reference marker
- compact parse/serialize roundtrip is deterministic
- replay snapshot is stable across repeated runs
