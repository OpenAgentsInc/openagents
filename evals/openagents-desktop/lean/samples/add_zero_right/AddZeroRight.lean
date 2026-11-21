-- Bundled zero-Mathlib Lean 4 smoke theorem for OpenAgents Desktop Gym (#13944)
theorem add_zero_right (n : Nat) : n + 0 = n := by
  rfl

theorem add_comm_nat (a b : Nat) : a + b = b + a := by
  induction b with
  | zero =>
    rw [Nat.add_zero, Nat.zero_add]
  | succ k ih =>
    rw [Nat.add_succ, Nat.succ_add, ih]
