[2024-10-29 03:46:48] testing.INFO: [BedrockMessageConverter] Converting prompt to Bedrock chat messages {"prompt":"[
    {
        \"role\": \"user\",
        \"content\": \"Open the README on the openagentsinc\\/openagents main branch and summarize in 1 sentence.\"
    },
    {
        \"role\": \"assistant\",
        \"content\": \" \",
        \"toolInvocations\": [
            {
                \"state\": \"result\",
                \"toolCallId\": \"tooluse_4-7M_FtMSIOPt-b_TuezDg\",
                \"toolName\": \"view_file\",
                \"args\": {
                    \"owner\": \"openagentsinc\",
                    \"repo\": \"openagents\",
                    \"path\": \"README.md\",
                    \"branch\": \"main\"
                },
                \"result\": {
                    \"type\": \"tool_call\",
                    \"value\": {
                        \"toolCallId\": \"tooluse_4-7M_FtMSIOPt-b_TuezDg\",
                        \"toolName\": \"view_file\",
                        \"args\": {
                            \"owner\": \"openagentsinc\",
                            \"repo\": \"openagents\",
                            \"path\": \"README.md\",
                            \"branch\": \"main\"
                        },
                        \"result\": {
                            \"success\": true,
                            \"content\": \"# OpenAgents v3\\n\\nOpenAgents is a platform for building, selling and using AI agents.\",
                            \"sha\": \"b5efe0a6ba5fda0bdeaf5d7f79f21d78e88aca10\",
                            \"summary\": \"Viewed file README.md in openagentsinc\\/openagents on branch main\",
                            \"details\": \"File contents have been retrieved from GitHub\"
                        }
                    }
                }
            }
        ]
    }
]"}
[2024-10-29 03:46:48] testing.INFO: Converted messages {"messages":[{"role":"user","content":[{"text":"Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence."}]},{"role":"assistant","content":[{"text":" "}]},{"role":"user","content":[{"text":"Continue."}]}]}
