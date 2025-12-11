# TestGen Protocol Improvement Plan

## Problem Statement

The TestGen E2E test with Haiku 4.5 (2024-12-11) revealed a critical gap:

**Failure**: Claude wrote 28 tests that all passed, but TB2 verification failed.

**Root Cause**: The instruction said "ensure that valid dates and IPv4 addresses are not immediately preceded or followed by alphanumeric characters" - a constraint applying to TWO entities. Claude only tested this for dates, not IPs.

**The Pattern**: When constraints apply to multiple entities, Claude may only test for one.

## Generalizable Principles Identified

### 1. ENTITY EXTRACTION
Before writing tests, explicitly list ALL entities being validated/processed.
- For regex-log: "dates" AND "IPv4 addresses"

### 2. CONSTRAINT-ENTITY MATRIX
Map each constraint to ALL entities it applies to:

| Constraint | Dates | IPs |
|------------|-------|-----|
| Format validation | ✓ | ✓ |
| Value ranges | ✓ | ✓ |
| **Word boundaries** | ✓ | ✓ | ← MISSED!
| Multiple handling | ✓ | - |
| Leading zeros | - | ✓ |

### 3. CROSS-PRODUCT TESTING
If constraint C applies to entities E1, E2, ... En → write N tests (one per entity).

### 4. "APPLIES TO ALL" DETECTION
Trigger phrases that signal multi-entity constraints:
- "dates and IPv4 addresses must not be..."
- "ensure that A and B are..."
- "valid X and Y should..."

### 5. NEGATIVE TEST COMPLETENESS
For every "must not" constraint, ensure negative tests for ALL mentioned entities.

## Solution: Add Entity-Constraint Mapping Phase

**Old workflow**: DESCRIBE → WRITE TESTS → ITERATE

**New workflow**: DESCRIBE → **MAP ENTITIES & CONSTRAINTS** → WRITE TESTS → ITERATE

The MAP phase forces explicit enumeration that prevents missing constraint coverage.

## Files to Modify

### 1. `crates/gym/src/mechacoder/testgen_wrapper.rs`

Update `wrap_instruction()` to add the new MAP phase:

```rust
// Add new phase between DESCRIBE and WRITE TESTS:

### Step 1.5: MAP ENTITIES & CONSTRAINTS (Required before writing tests)

Create a constraint coverage matrix:

```markdown
### ENTITIES IDENTIFIED
1. [Entity 1 from task description]
2. [Entity 2 from task description]
...

### CONSTRAINTS IDENTIFIED
1. [Constraint 1]
2. [Constraint 2]
...

### CONSTRAINT-ENTITY MATRIX
| Constraint | Entity 1 | Entity 2 | ... |
|------------|----------|----------|-----|
| Constraint 1 | ✓ or - | ✓ or - | ... |
| Constraint 2 | ✓ or - | ✓ or - | ... |
...

### REQUIRED TESTS (one per ✓ cell)
- test_entity1_constraint1
- test_entity2_constraint1 (if ✓)
- test_entity1_constraint2
...
```

CRITICAL: Every ✓ in the matrix MUST have a corresponding test in testgen_tests.py.
```

### 2. `.claude/skills/testgen-protocol/SKILL.md`

Add the Entity-Constraint Mapping methodology as a required step.

## Implementation Details

### Step 1.5 Content to Add to Wrapper

```
### Step 1.5: MAP ENTITIES & CONSTRAINTS

Before writing ANY tests, create a coverage matrix:

**ENTITIES**: List every distinct thing being validated/matched/processed
- Look for nouns that have constraints applied to them
- Example: "dates", "IPv4 addresses", "usernames"

**CONSTRAINTS**: List every rule/requirement from the description
- Look for "must", "should", "ensure", "valid", "not"
- Example: "must be in format X", "not preceded by Y"

**MATRIX**: For EACH constraint, mark which entities it applies to

**KEY INSIGHT**: When the description says "A and B must [constraint]",
this means the constraint applies to BOTH A and B. You need tests for BOTH.

Example from regex-log task:
> "ensure that valid dates and IPv4 addresses are not immediately preceded
> or followed by alphanumeric characters"

This constraint applies to:
- ✓ dates (need test: date preceded/followed by alphanumeric)
- ✓ IPv4 addresses (need test: IP preceded/followed by alphanumeric)

If you only test dates, you will miss bugs in IP handling!
```

### Skill File Changes

Add new section after Phase 1 DESCRIBE:

```markdown
## Phase 1.5: MAP ENTITIES & CONSTRAINTS (Required)

Before writing tests, create an explicit coverage map:

### Step 1: Extract Entities
List ALL nouns that have requirements applied to them:
```
ENTITIES:
1. [entity1] - what is being matched/validated/processed
2. [entity2] - another thing being matched/validated/processed
...
```

### Step 2: Extract Constraints
List ALL rules/requirements from the description:
```
CONSTRAINTS:
1. [constraint1] - a rule about format/value/boundary/etc
2. [constraint2] - another rule
...
```

### Step 3: Build the Matrix
For each constraint, determine which entities it applies to:
```
| Constraint | Entity1 | Entity2 | ... |
|------------|---------|---------|-----|
| constraint1 | ✓ | ✓ | ... |
| constraint2 | ✓ | - | ... |
```

### Step 4: Enumerate Required Tests
For each ✓ in the matrix, you MUST write a test:
```
REQUIRED TESTS:
- test_entity1_constraint1
- test_entity2_constraint1  ← Don't miss this!
- test_entity1_constraint2
...
```

### CRITICAL PATTERN: "A and B must..."

When you see phrases like:
- "dates and IPv4 addresses must not be..."
- "ensure that X and Y are..."
- "valid A and B should..."

This means the constraint applies to MULTIPLE entities.
You MUST test the constraint for EACH entity separately.

Example failure mode (what we're preventing):
> "ensure dates and IPs are not adjacent to alphanumerics"

❌ WRONG: Only test date boundaries
✓ RIGHT: Test date boundaries AND IP boundaries
```

## Verification

After implementation, re-run the regex-log test. The new MAP phase should produce:

```
### ENTITIES IDENTIFIED
1. dates (YYYY-MM-DD format)
2. IPv4 addresses (decimal notation)

### CONSTRAINTS IDENTIFIED
1. dates must be in YYYY-MM-DD format
2. IPv4 must have valid octets (0-255)
3. IPv4 must have no leading zeros
4. match only last date in line
5. only match if line contains IPv4
6. dates AND IPs must not be adjacent to alphanumerics  ← KEY!

### CONSTRAINT-ENTITY MATRIX
| Constraint | Dates | IPs |
|------------|-------|-----|
| Format validation | ✓ | ✓ |
| Value ranges | ✓ | ✓ |
| Leading zeros | - | ✓ |
| Match last only | ✓ | - |
| Require in line | - | ✓ |
| No alphanumeric adj | ✓ | ✓ |  ← BOTH!

### REQUIRED TESTS
- test_date_format_valid
- test_ip_format_valid
- test_date_value_ranges (month 01-12, day 01-31)
- test_ip_value_ranges (octet 0-255)
- test_ip_no_leading_zeros
- test_match_last_date_only
- test_require_ip_in_line
- test_date_no_alphanumeric_adjacent  ← Claude had this
- test_ip_no_alphanumeric_adjacent    ← Claude MISSED this
```

## Success Criteria

1. MAP phase appears in Claude's output before WRITE TESTS
2. Entity-Constraint matrix is explicitly written
3. Tests cover all ✓ cells in the matrix
4. Re-running regex-log test passes TB2 verification

## Anti-Cheating Verification

This improvement is GENERALIZABLE because:
- It doesn't mention specific tasks or task IDs
- It doesn't inject domain knowledge (regex syntax, IP formats)
- It teaches a PROCESS (entity-constraint mapping) not a SOLUTION
- The same technique applies to ANY task with multiple entities


