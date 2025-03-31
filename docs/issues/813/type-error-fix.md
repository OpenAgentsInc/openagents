# TypeScript Error Fix

## Problem

The codebase had a TypeScript error in `usePersistentChat.ts` when running `yarn t`:

```
../../packages/core/src/chat/usePersistentChat.ts:130:35 - error TS2345: Argument of type '{ id: string | undefined; maxSteps: number; api: string; body: { debug: boolean; format: string; model: string; }; headers: { 'Content-Type': string; Accept: string; } | { 'Content-Type': string; Accept: string; } | { ...; }; ... 13 more ...; fetch?: FetchFunction; }' is not assignable to parameter of type 'UseChatOptions & { key?: string | undefined; experimental_prepareRequestBody?: ((options: { id: string; messages: UIMessage[]; requestData?: JSONValue | undefined; requestBody?: object | undefined; }) => unknown) | undefined; experimental_throttle?: number | undefined; maxSteps?: number | undefined; }'.
  Type '{ id: string | undefined; maxSteps: number; api: string; body: { debug: boolean; format: string; model: string; }; headers: { 'Content-Type': string; Accept: string; } | { 'Content-Type': string; Accept: string; } | { ...; }; ... 13 more ...; fetch?: FetchFunction; }' is not assignable to type 'UseChatOptions'.
    Types of property 'streamProtocol' are incompatible.
      Type 'string' is not assignable to type '"data" | "text" | undefined'.

130   const vercelChatState = useChat(customOptions);
                                      ~~~~~~~~~~~~~
```

The error indicates that TypeScript was inferring a general `string` type for the `streamProtocol` property when the `useChat` hook expects it to be specifically either `"data"` or `"text"` or `undefined`.

## Fix

Added a type assertion to ensure TypeScript recognizes `'data'` as the literal type `'data'` rather than the general `string` type:

```typescript
// Before
streamProtocol: 'data',

// After 
streamProtocol: 'data' as 'data',
```

This change helps TypeScript understand that we're specifically using the `'data'` protocol, which is one of the allowed values in the type definition for `UseChatOptions.streamProtocol`.

## Impact

- Fixed the TypeScript error that was occurring during the type-checking process (`yarn t`)
- Ensured proper type safety for the API call
- No functional changes to the behavior of the code