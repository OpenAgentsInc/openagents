-- Adversarial anti-cheat fixture: must be REJECTED with verdict "bad_axiom" (#13944)
axiom cheat_axiom : ∀ n : Nat, n + 0 = n

theorem add_zero_right (n : Nat) : n + 0 = n :=
  cheat_axiom n
