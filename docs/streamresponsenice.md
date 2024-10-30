[2024-10-30 02:57:13] testing.INFO: [BedrockMessageConverter] Converting prompt to Bedrock chat messages {"prompt":"[
    {
        \"role\": \"user\",
        \"content\": \"Say hello world.\"
    }
]"}
[2024-10-30 02:57:13] testing.INFO: Converted messages {"messages":[{"role":"user","content":[{"text":"Say hello world."}]}]}
[2024-10-30 02:57:13] testing.INFO: Retrieved definition for App\Tools\ViewFile {"definition":{"type":"function","function":{"name":"view_file","description":"View file contents at path","parameters":{"type":"object","properties":{"path":{"type":"string","description":"The full path of the file to view"},"owner":{"type":"string","description":"The owner of the repository"},"repo":{"type":"string","description":"The name of the repository"},"branch":{"type":"string","description":"The branch to view the file from"}},"required":["path","owner","repo"]}}}}
[2024-10-30 02:57:13] testing.INFO: Retrieved definition for App\Tools\ViewFolder {"definition":{"name":"view_folder","description":"View file/folder hierarchy at path (one level deep)","parameters":{"type":"object","properties":{"path":{"type":"string","description":"The path to view the folder contents"},"branch":{"type":"string","description":"The branch to view the folder contents from"},"owner":{"type":"string","description":"The owner of the repository"},"repo":{"type":"string","description":"The name of the repository"}},"required":["path","owner","repo"]}}}
[2024-10-30 02:57:13] testing.INFO: Retrieved definition for App\Tools\RewriteFile {"definition":{"name":"rewrite_file","description":"Rewrites the contents of a file at the given path","parameters":{"type":"object","properties":{"path":{"type":"string","description":"The path of the file to rewrite"},"content":{"type":"string","description":"The new content to write to the file"},"owner":{"type":"string","description":"The owner of the repository"},"repo":{"type":"string","description":"The name of the repository"},"branch":{"type":"string","description":"The branch to update"}},"required":["path","content","owner","repo"]}}}
[2024-10-30 02:57:13] testing.INFO: Retrieved definition for App\Tools\CreateFile {"definition":{"name":"create_file","description":"Creates a new file at the given path with the provided content","parameters":{"type":"object","properties":{"path":{"type":"string","description":"The path where the new file should be created"},"content":{"type":"string","description":"The content of the new file"},"owner":{"type":"string","description":"The owner of the repository"},"repo":{"type":"string","description":"The name of the repository"},"branch":{"type":"string","description":"The branch to create the file on. If not provided, the default branch from the context will be used."}},"required":["path","content","owner","repo"]}}}
[2024-10-30 02:57:13] testing.INFO: Generated tool definitions {"toolDefinitions":[{"toolSpec":{"name":"view_file","description":"View file contents at path","inputSchema":{"json":{"type":"object","properties":{"path":{"type":"string","description":"The full path of the file to view"},"owner":{"type":"string","description":"The owner of the repository"},"repo":{"type":"string","description":"The name of the repository"},"branch":{"type":"string","description":"The branch to view the file from"}},"required":["path","owner","repo"]}}}},{"toolSpec":{"name":"view_folder","description":"View file/folder hierarchy at path (one level deep)","inputSchema":{"json":{"type":"object","properties":{"path":{"type":"string","description":"The path to view the folder contents"},"branch":{"type":"string","description":"The branch to view the folder contents from"},"owner":{"type":"string","description":"The owner of the repository"},"repo":{"type":"string","description":"The name of the repository"}},"required":["path","owner","repo"]}}}},{"toolSpec":{"name":"rewrite_file","description":"Rewrites the contents of a file at the given path","inputSchema":{"json":{"type":"object","properties":{"path":{"type":"string","description":"The path of the file to rewrite"},"content":{"type":"string","description":"The new content to write to the file"},"owner":{"type":"string","description":"The owner of the repository"},"repo":{"type":"string","description":"The name of the repository"},"branch":{"type":"string","description":"The branch to update"}},"required":["path","content","owner","repo"]}}}},{"toolSpec":{"name":"create_file","description":"Creates a new file at the given path with the provided content","inputSchema":{"json":{"type":"object","properties":{"path":{"type":"string","description":"The path where the new file should be created"},"content":{"type":"string","description":"The content of the new file"},"owner":{"type":"string","description":"The owner of the repository"},"repo":{"type":"string","description":"The name of the repository"},"branch":{"type":"string","description":"The branch to create the file on. If not provided, the default branch from the context will be used."}},"required":["path","content","owner","repo"]}}}}]}
[2024-10-30 02:57:13] testing.INFO: We have prepared the request body {"requestBody":{"modelId":"anthropic.claude-3-sonnet-20240229-v1:0","contentType":"application/json","accept":"application/json","anthropic_version":"bedrock-2023-05-31","messages":[{"role":"user","content":[{"text":"Say hello world."}]}],"max_tokens":2200,"temperature":0.7,"top_p":1,"toolConfig":{"tools":[{"toolSpec":{"name":"view_file","description":"View file contents at path","inputSchema":{"json":{"type":"object","properties":{"path":"Over 9 levels deep, aborting normalization","owner":"Over 9 levels deep, aborting normalization","repo":"Over 9 levels deep, aborting normalization","branch":"Over 9 levels deep, aborting normalization"},"required":["Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization"]}}}},{"toolSpec":{"name":"view_folder","description":"View file/folder hierarchy at path (one level deep)","inputSchema":{"json":{"type":"object","properties":{"path":"Over 9 levels deep, aborting normalization","branch":"Over 9 levels deep, aborting normalization","owner":"Over 9 levels deep, aborting normalization","repo":"Over 9 levels deep, aborting normalization"},"required":["Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization"]}}}},{"toolSpec":{"name":"rewrite_file","description":"Rewrites the contents of a file at the given path","inputSchema":{"json":{"type":"object","properties":{"path":"Over 9 levels deep, aborting normalization","content":"Over 9 levels deep, aborting normalization","owner":"Over 9 levels deep, aborting normalization","repo":"Over 9 levels deep, aborting normalization","branch":"Over 9 levels deep, aborting normalization"},"required":["Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization"]}}}},{"toolSpec":{"name":"create_file","description":"Creates a new file at the given path with the provided content","inputSchema":{"json":{"type":"object","properties":{"path":"Over 9 levels deep, aborting normalization","content":"Over 9 levels deep, aborting normalization","owner":"Over 9 levels deep, aborting normalization","repo":"Over 9 levels deep, aborting normalization","branch":"Over 9 levels deep, aborting normalization"},"required":["Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization","Over 9 levels deep, aborting normalization"]}}}}]}}}
[2024-10-30 02:57:14] testing.INFO: array (
  'messageStart' =>
  array (
    'role' => 'assistant',
  ),
)
[2024-10-30 02:57:14] testing.INFO: array (
  'contentBlockDelta' =>
  array (
    'delta' =>
    array (
      'text' => 'Hello',
    ),
    'contentBlockIndex' => 0,
  ),
)
[2024-10-30 02:57:14] testing.INFO: array (
  'contentBlockDelta' =>
  array (
    'delta' =>
    array (
      'text' => ' worl',
    ),
    'contentBlockIndex' => 0,
  ),
)
[2024-10-30 02:57:14] testing.INFO: array (
  'contentBlockDelta' =>
  array (
    'delta' =>
    array (
      'text' => 'd.',
    ),
    'contentBlockIndex' => 0,
  ),
)
[2024-10-30 02:57:14] testing.INFO: array (
  'contentBlockStop' =>
  array (
    'contentBlockIndex' => 0,
  ),
)
[2024-10-30 02:57:14] testing.INFO: array (
  'messageStop' =>
  array (
    'stopReason' => 'end_turn',
  ),
)
[2024-10-30 02:57:14] testing.INFO: array (
  'metadata' =>
  array (
    'usage' =>
    array (
      'inputTokens' => 789,
      'outputTokens' => 6,
      'totalTokens' => 795,
    ),
    'metrics' =>
    array (
      'latencyMs' => 620,
    ),
  ),
)
