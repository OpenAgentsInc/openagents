# RxDB Setup Learnings

## Schema Validation in Development Mode

When using RxDB with development mode enabled (`RxDBDevModePlugin`), there are some important requirements and considerations:

1. **Mandatory Schema Validation**
   - Dev mode requires schema validation at the storage level
   - This is enforced to catch schema violations early and prevent hard-to-debug issues
   - Error code `DVM1` will be thrown if validation is not properly configured

2. **Validation Options**
   - Two main validation plugins are available:
     - `validate-ajv`: Uses AJV for validation but requires `unsafe-eval` in CSP
     - `validate-z-schema`: Alternative that works with strict CSP (no eval)
   - For applications with strict Content Security Policy, use `validate-z-schema`

3. **Implementation Pattern**
```typescript
import { wrappedValidateZSchemaStorage } from 'rxdb/plugins/validate-z-schema';

// Wrap the storage with validation
const storage = wrappedValidateZSchemaStorage({
  storage: getRxStorageDexie()
});
```

## Content Security Policy Considerations

1. **AJV vs Z-Schema**
   - AJV validator uses `eval()` which conflicts with strict CSP
   - Z-Schema provides same validation without requiring `unsafe-eval`
   - Choose validator based on your CSP requirements

2. **CSP Requirements**
   - If your CSP includes `script-src 'self'`
   - And doesn't allow `unsafe-eval`
   - You must use `validate-z-schema`

## Best Practices

1. **Development Mode**
   - Always enable dev mode during development
   - Helps catch schema violations early
   - Provides better error messages and debugging

2. **Schema Validation**
   - Define strict schemas with proper types and constraints
   - Include `maxLength` for string fields
   - Define proper indexes for better query performance

3. **Error Handling**
   - Handle database creation errors gracefully
   - Log errors for debugging
   - Provide user-friendly error messages

## Common Issues and Solutions

1. **DVM1 Error**
   - Cause: Missing schema validation in dev mode
   - Solution: Wrap storage with a validator

2. **CSP Eval Error**
   - Cause: AJV validator using eval
   - Solution: Switch to z-schema validator

3. **Database Creation**
   - Ensure proper initialization order
   - Handle singleton pattern correctly
   - Clean up resources when needed
