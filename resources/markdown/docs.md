# Developer documentation

OpenAgents.com is a platform for building and commercializing AI agents using shared databases of skills and knowledge.

It is under active development. All code is open source under AGPL3 [here](https://github.com/OpenAgentsInc/openagents).

## Concepts

* The primary chat interface at OpenAgents.com allows users to interact with a variety of basic chat models and soon our
  meta-agent called "OpenAgents", which
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

## Definitions

* **Agent** - An AI entity executing defined tasks
* **Thread** - A message chain between user and agent
* **Message** - A single communication in a thread
* **File** - Document processed or created by agents
* **Run** - The active execution of an agent flow
* **Flow** - A sequence of nodes
* **Node** - An individual task within a flow
* **Plugin** - A WebAssembly binary extending agent functionality


## How This Works (MVP v1 - RAG Agent)


![f1 (2)](https://github.com/OpenAgentsInc/openagents/assets/93095163/390342ec-c7cb-4483-a671-995eb6daac7a)



The user builds the Agent from the Agent Builder UI (MVP version in the img).

The user provides name and description of what the agent does, and the specific parameters it needs to operate, that are "Instructions" and "Knowledge".

As the user save the Agent build, they can evoke it (semantic routes) and interact with it via Chat UI.

On the back-end, the Agent will send an event template to the [Nostr implementation](https://github.com/OpenAgentsInc/openagents/wiki/Nostr-integration) for the execution.

The communication between the OpenAgents platform Laravel codebase and Nostr are performed through a [gRPC client](https://github.com/OpenAgentsInc/openagents/wiki/Agent-Builder-MVP-Spec#laravel---grpc-connection) intermediary.

The event template is compiled with the following params:

* `poolAddress` = the host

* `query` = the LLM generated rag query from the user input ("Instructions") + chat history (**Thread**)

* `documents` =  knowledge files as array of URLs (**File**)

* `k` = how many chunks to return

* `max_tokens` = numbers of tokens for text chunk

* `overlap` = overlap between chunks

* `encryptFor` = encrypt for a specific provider, so it can see it’s content


### RAG Agent Pipeline


![NVIDIA-RAG-diagram-scaled](https://github.com/OpenAgentsInc/openagents/assets/93095163/fa848c08-2c02-47bf-a8bd-93053a5e22bd)
[source](https://blogs.nvidia.com/blog/what-is-retrieval-augmented-generation/)


The above is a representation of a RAG Agent pipeline.

OpenAgents' RAG Agent handle these phases with the following plugins/standalone nodes:

* Retrieve Document: [Openagents Document Retrieval Node](https://github.com/riccardobl/openagents-document-retrieval)

* Embedding model: [Openagents Embeddings Node](https://github.com/riccardobl/openagents-embeddings)

* Vector DB: [Openagents Search Node](https://github.com/riccardobl/openagents-search)

These three nodes are coordinated by the [RAG Coordinator](https://github.com/riccardobl/openagents-rag-coordinator-plugin) Extism [plugin](https://github.com/OpenAgentsInc/openagents/wiki/Plugins).

## Getting paid for agent upgrades

Payouts are Bitcoin only, paid via the Lightning network.

For now this process is semi-manual.

1. Write an Extism plugin using one of their eight [plugin development kits](https://extism.org/docs/concepts/pdk/). It
   must
   expose one function called 'run' that takes a single string parameter and returns a single string. (Use JSON
   stringification as needed for larger objects.)
2. Upload the code to a GitHub repo.
3. Send us a [DM us on X](https://twitter.com/OpenAgentsInc) (@OpenAgentsInc) with 1) a public-facing name and
   description for the plugin, 2) the link to the GitHub repo and 3) your
   [Lightning address](https://lightningaddress.com/)

The first 100 developers to submit a plugin we add to OpenAgents will receive ₿1M each (~$600 USD as of 4/17).

You can see previously submitted plugins [here](/plugins).

## API overview

We will soon release a developer API.

You can watch a video introducing it here:

<blockquote class="twitter-tweet" data-media-max-width="560"><p lang="en" dir="ltr">Episode 85: API Design<br><br>We introduce the OpenAgents API and compare it to the OpenAI Assistants API.<br><br>re: <a href="https://t.co/1RDnbvE7yO">https://t.co/1RDnbvE7yO</a> <a href="https://t.co/0cotmwx1BS">pic.twitter.com/0cotmwx1BS</a></p>&mdash; OpenAgents ⚡ (@OpenAgentsInc) <a href="https://twitter.com/OpenAgentsInc/status/1762596179643371596?ref_src=twsrc%5Etfw">February 27, 2024</a></blockquote>


To get early access, [DM us on X](https://twitter.com/OpenAgentsInc) (@OpenAgentsInc) with how you'd like to use the
API. We'll start sending invites in late March.
