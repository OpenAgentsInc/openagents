-- OpenAgents Desktop Lean 4 Axiom & Statement Verifier Harness (Epic #13943, #13944)
-- Usage: lake env lean -DautoImplicit=false OpenAgentsVerify.lean
-- Paired with #authority coax/lib/lean/policy.coax which accepts ONLY:
--   {propext, Quot.sound, Classical.choice}
-- and rejects sorryAx, Lean.ofReduceBool, Lean.trustCompiler, and any custom axiom.

import Lean

open Lean

def checkAxiomsOnlyStandard (env : Environment) (declName : Name) : IO Unit := do
  let axioms := CollectAxioms.collect declName |>.run env |>.2.axioms
  IO.println s!"'{declName}' depends on axioms: {axioms.toList}"
