# Multiproviders

We need no dependence on one or a few providers.

Fallbacks

* OpenAI
* Other OpenAI-compatible APIs
* Non-OAI APIs we want the firepower of, e.g. Gemini Pro 1.5
* Open models
* GPUtopia / swarm compute as ultimate fallback
  * And grows in relevance/applicability as more capacity comes online / more models supported etc

Need various failover algorithms

Also some things better for some providers

Failover like --
When I request an embedding, try to get one from a certain gateway - if it fails, go to next
I can maybe specify preferences like, prefer gputopia network for embedding, but if u wait longer than 1s, go to next . . .
