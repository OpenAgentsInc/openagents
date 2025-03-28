 The "Unauthorized" error is happening with OpenRouter, not with GitHub tools. The new log confirms the problem:

  💥 streamText onError callback: Unauthorized AI_APICallError: Unauthorized

  This error occurs immediately when trying to connect to OpenRouter, before any GitHub tools can be executed. The issue is with the
  OpenRouter API authentication.

  I've made a change to add the OpenRouter API key explicitly in a header called 'OR-API-KEY' while also adding logging to verify the API
   key is present and what format it's in.

  The error suggests that either:
  1. The OPENROUTER_API_KEY environment variable might be invalid or expired
  2. The way it's being passed to the OpenRouter provider isn't correct

  We need to verify:
  1. If the OpenRouter API key is valid and active
  2. If the createOpenRouter({ apiKey }) configuration is properly passing the key
  3. If the API key format matches what OpenRouter expects

  The logs from the next run will show if the API key is present and its format, which should help confirm if this is the issue.
