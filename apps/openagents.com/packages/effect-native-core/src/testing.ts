// Test-only re-export so consumers pin the SAME effect instance the catalog
// runtime uses (this package's `effect` dependency) when driving TestClock etc.
// Importing `effect/testing` directly from a consumer can resolve a different
// hoisted effect copy and produce cross-instance type mismatches.
export { TestClock } from 'effect/testing'
