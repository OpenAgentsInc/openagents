Request:
```
{
  "messages": [
    {
      "role": "user",
      "content": "Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence."
    }
  ],
  "model": "anthropic.claude-3-5-sonnet-20241022-v2:0",
  "tools": [
    "view_file",
    "view_hierarchy",
    "scrape_webpage",
    "create_file",
    "rewrite_file",
    "delete_file"
  ],
  "threadId": "jh71jz5vdgmeppjyeyh4kb8xvs73hy8a",
  "repoOwner": "OpenAgentsInc",
  "repoName": "openagents",
  "repoBranch": "chat",
  "githubToken": null
}
```

```
0:" branch."
9:{"toolCallId":"tooluse_ayhWUyeCTyK_oR_D0vdX0Q","toolName":"view_file","args":{"path":"README.md","owner":"OpenAgentsInc","repo":"openagents","branch":"main"}}
d:{"finishReason":"tool-calls","usage":{"promptTokens":2500,"completionTokens":94}}
a:{"toolCallId":"tooluse_ayhWUyeCTyK_oR_D0vdX0Q","result":{"success":true,"content":"# OpenAgents v3\n\nOpenAgents is a platform for building, selling and using AI agents.\n\nWe are building v3 from scratch here.\n\n## Tech Stack\n\n- Laravel\n- React\n- Inertia\n- Tailwind\n\n## Previous work\n\n- OpenAgents v1 (Laravel/Livewire) has moved to the [v1 branch](https://github.com/OpenAgentsInc/openagents/tree/v1).\n- OpenAgents v2 (NextJS) has its own repo [here](https://github.com/OpenAgentsInc/v2)\n\n## Resources\n\n- [Follow us on X](https://x.com/OpenAgentsInc)\n- [Wiki](https://github.com/OpenAgentsInc/openagents/wiki)\n- [Stacker News Community](https://stacker.news/~openagents)\n\n## Video series\n\nWe've chronicled most of the development of this platform over multiple months and 100+ videos on X.\n\nSee [episode one](https://twitter.com/OpenAgentsInc/status/1721942435125715086) or\nthe [full episode list](https://github.com/OpenAgentsInc/openagents/wiki/Video-Series).\n","summary":"Viewed README.md in OpenAgentsInc/openagents on branch main","details":"File contents of README.md in OpenAgentsInc/openagents on branch main have been retrieved:\n\n# OpenAgents v3\n\nOpenAgents is a platform for building, selling and using AI agents.\n\nWe are building v3 from scratch here.\n\n## Tech Stack\n\n- Laravel\n- React\n- Inertia\n- Tailwind\n\n## Previous work\n\n- OpenAgents v1 (Laravel/Livewire) has moved to the [v1 branch](https://github.com/OpenAgentsInc/openagents/tree/v1).\n- OpenAgents v2 (NextJS) has its own repo [here](https://github.com/OpenAgentsInc/v2)\n\n## Resources\n\n- [Follow us on X](https://x.com/OpenAgentsInc)\n- [Wiki](https://github.com/OpenAgentsInc/openagents/wiki)\n- [Stacker News Community](https://stacker.news/~openagents)\n\n## Video series\n\nWe've chronicled most of the development of this platform over multiple months and 100+ videos on X.\n\nSee [episode one](https://twitter.com/OpenAgentsInc/status/1721942435125715086) or\nthe [full episode list](https://github.com/OpenAgentsInc/openagents/wiki/Video-Series).\n"}}
```

Request #2:
```
{
  "messages": [
    {
      "role": "user",
      "content": "Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence."
    },
    {
      "role": "assistant",
      "content": "Let me fetch the README from the main branch.",
      "toolInvocations": [
        {
          "state": "result",
          "toolCallId": "tooluse_ayhWUyeCTyK_oR_D0vdX0Q",
          "toolName": "view_file",
          "args": {
            "path": "README.md",
            "owner": "OpenAgentsInc",
            "repo": "openagents",
            "branch": "main"
          },
          "result": {
            "success": true,
            "content": "# OpenAgents v3\n\nOpenAgents is a platform for building, selling and using AI agents.\n\nWe are building v3 from scratch here.\n\n## Tech Stack\n\n- Laravel\n- React\n- Inertia\n- Tailwind\n\n## Previous work\n\n- OpenAgents v1 (Laravel/Livewire) has moved to the [v1 branch](https://github.com/OpenAgentsInc/openagents/tree/v1).\n- OpenAgents v2 (NextJS) has its own repo [here](https://github.com/OpenAgentsInc/v2)\n\n## Resources\n\n- [Follow us on X](https://x.com/OpenAgentsInc)\n- [Wiki](https://github.com/OpenAgentsInc/openagents/wiki)\n- [Stacker News Community](https://stacker.news/~openagents)\n\n## Video series\n\nWe've chronicled most of the development of this platform over multiple months and 100+ videos on X.\n\nSee [episode one](https://twitter.com/OpenAgentsInc/status/1721942435125715086) or\nthe [full episode list](https://github.com/OpenAgentsInc/openagents/wiki/Video-Series).\n",
            "summary": "Viewed README.md in OpenAgentsInc/openagents on branch main",
            "details": "File contents of README.md in OpenAgentsInc/openagents on branch main have been retrieved:\n\n# OpenAgents v3\n\nOpenAgents is a platform for building, selling and using AI agents.\n\nWe are building v3 from scratch here.\n\n## Tech Stack\n\n- Laravel\n- React\n- Inertia\n- Tailwind\n\n## Previous work\n\n- OpenAgents v1 (Laravel/Livewire) has moved to the [v1 branch](https://github.com/OpenAgentsInc/openagents/tree/v1).\n- OpenAgents v2 (NextJS) has its own repo [here](https://github.com/OpenAgentsInc/v2)\n\n## Resources\n\n- [Follow us on X](https://x.com/OpenAgentsInc)\n- [Wiki](https://github.com/OpenAgentsInc/openagents/wiki)\n- [Stacker News Community](https://stacker.news/~openagents)\n\n## Video series\n\nWe've chronicled most of the development of this platform over multiple months and 100+ videos on X.\n\nSee [episode one](https://twitter.com/OpenAgentsInc/status/1721942435125715086) or\nthe [full episode list](https://github.com/OpenAgentsInc/openagents/wiki/Video-Series).\n"
          }
        }
      ]
    }
  ],
  "model": "anthropic.claude-3-5-sonnet-20241022-v2:0",
  "tools": [
    "view_file",
    "view_hierarchy",
    "scrape_webpage",
    "create_file",
    "rewrite_file",
    "delete_file"
  ],
  "threadId": "jh71jz5vdgmeppjyeyh4kb8xvs73hy8a",
  "repoOwner": "OpenAgentsInc",
  "repoName": "openagents",
  "repoBranch": "chat",
  "githubToken": null
}
```

Response #2:
```
0:"\n\nOpen"
0:"Agents is"
0:" a Laravel"
0:"/"
0:"React-"
0:"based platform for building"
0:", selling and using"
0:" AI agents,"
0:" currently"
0:" being"
0:" rebuilt"
0:" from scratch as"
0:" v3 "
0:"with"
0:" development"
0:" progress"
0:" documente"
0:"d through"
0:" "
0:"100+ videos on"
0:" X."
d:{"finishReason":"stop","usage":{"promptTokens":3350,"completionTokens":45}}
```
