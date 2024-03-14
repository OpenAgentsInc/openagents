# Developer documentation

OpenAgents.com is a platform for building and commercializing AI agents using shared databases of skills and knowledge.

## Concepts

* The primary chat interface at OpenAgents.com allows users to interact with our meta-agent called "OpenAgents", which
  is
  composed of multiple
  Agents.

* Agents are built from interchangeable building blocks called Nodes which are organized into sequences called Flows.

* Example Nodes include: API endpoints, conditional logic, data parsing, and third-party integrations.

* Nodes can be created by community developers by uploading WASM plugins.

* Plugins are a special type of Node that allows for bespoke operations and enhanced capabilities.

* Each Node may have an associated fee, payable to its creator upon use.

* Nodes can reference an agent's Files.

* Users converse with Agents in conversations that are Threads of Messages.

* A Run is an instance of executing a Flow.

## API Overview

We will soon release a developer API.

You can watch a video introducing it here:

<blockquote class="twitter-tweet" data-media-max-width="560"><p lang="en" dir="ltr">Episode 85: API Design<br><br>We introduce the OpenAgents API and compare it to the OpenAI Assistants API.<br><br>re: <a href="https://t.co/1RDnbvE7yO">https://t.co/1RDnbvE7yO</a> <a href="https://t.co/0cotmwx1BS">pic.twitter.com/0cotmwx1BS</a></p>&mdash; OpenAgents ⚡ (@OpenAgentsInc) <a href="https://twitter.com/OpenAgentsInc/status/1762596179643371596?ref_src=twsrc%5Etfw">February 27, 2024</a></blockquote>


To get early access, [DM us on X](https://twitter.com/OpenAgentsInc) (@OpenAgentsInc) with how you'd like to use the
API. We'll start sending invites in late March.