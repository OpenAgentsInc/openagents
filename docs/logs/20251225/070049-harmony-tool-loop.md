# Harmony tool-call loop

- extended GPT-OSS session history to retain Harmony metadata (recipient/name/channel/content_type)
- added tool-call extraction + execution loop (`send_with_tools`) and tool result messages
- updated local-infer GPT-OSS path to use Harmony tool loop when `--tools` is enabled
- refreshed d-019 directive + status and GPT-OSS docs to reflect the change
