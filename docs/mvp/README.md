# March 14 MVP Spec

* Guests can interact with an "Agent Builder" agent.
    * Collect project scope
    * Want to commission an agent - pay bounty / fee fair?
    * Identify plugins needed to bring that to life
* Developers can add plugin nodes via the API.
    * For now plugins must take a string, return a string
    * To build:
        * Agent Builder
        * Chat Distiller
        * Agents requested by users
* Developer bounties available
* Thorough docs

## Milestones

* New Livewire agent flow must use the new data models and services
    * Agent Builder is specified via DB seeder as Flow of Nodes
    * Input is sent to new API endpoint/service that triggers a Run
    * Runs broadcast events with status updates

## Comms

* Homepage
    * Build your own AI agent
    * What do you want to accomplish?
    * I want my agent to...
* Blog
    * Launching our snowball
    * Two-sided marketplace: agent builders & users
        * Builders may be developers or not
    * All open-source. Hey, submit a PR or fork the code and build this yourself
    * Where are the agents you would pay for? They don't exist. We need to build them - together.
* Docs
* Bounties