⏺ I've implemented the suggested approach to debug the TypeError:

  1. Simplified to One Tool:
    - Modified extractToolDefinitions() to only return a single tool (create_issue)
    - Added filtering to skip all other tools during the mapping process
  2. Minimal Schema:
    - Created a minimal schema with just one property (repo) for the tool
    - Removed the required fields array to minimize complexity
    - Added detailed logging to show the exact schema being generated
  3. Re-enabled Tools:
    - Re-enabled the tools configuration in streamText() to test with our minimal schema
    - This will help determine if the TypeError was caused by complex tool schemas

  This approach drastically simplifies the tool schema to help isolate whether the content of the properties or the number of tools was
  causing the error. By reducing to a single tool with minimal properties, we can see if the TypeError about reading 'typeName' of
  undefined goes away.

  The additional logging will also help us see exactly what schema is being passed to streamText and whether it's correctly formatted
  according to what the SDK expects.
