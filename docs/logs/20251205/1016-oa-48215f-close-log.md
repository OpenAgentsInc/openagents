# 1016 - Close oa-48215f: Edit tool SDK naming

## Investigation
Task oa-48215f was marked as in_progress with no commits, but investigation revealed the work was ALREADY COMPLETE.

## Findings
Edit tool (src/tools/edit.ts) already has:
- ✅ SDK naming support (file_path, old_string, new_string) alongside internal naming
- ✅ replace_all parameter fully implemented
- ✅ All tests passing (4/4)
- ✅ Proper fallback logic: params.path ?? params.file_path

## Implementation Details
The work was completed in commit 16adaf938 (oa-pi10: enrich tool details metadata).

Schema accepts BOTH naming styles:
```typescript
const EditParametersSchema = S.Struct({
  path: S.optional(pathField),
  file_path: S.optional(pathField),
  oldText: S.optional(textField),
  old_string: S.optional(textField),
  newText: S.optional(replacementField),
  new_string: S.optional(replacementField),
  replace_all: S.optional(S.Boolean),
});
```

## Tests
All edit tool tests pass:
- SDK naming parameters work correctly
- replace_all functionality verified
- 4 tests, 10 expect() calls, 100% pass rate

## Task Status
Closed with commit reference 16adaf938.
