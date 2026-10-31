# 203. Tools as typed capabilities

> **Status: unimplemented — design draft.** Nothing here runs yet.

The model asks for tools by name and arguments. This chapter defines what a
tool is in Psionic: a typed capability with a schema, a guarded pipeline and
structured errors. Arguments are constrained while the model generates them,
so the harness never receives a malformed call.

## What this chapter covers

- A tool registry. Each tool's input and output schemas are versioned
  messages in `proto/`.
- A guarded pipeline with three stages: before, run and after. A stage can
  refuse, and a refusal is a structured error that the model sees.
- Constrained arguments. Chapter 53's grammar masks keep the model from
  emitting arguments that do not match the schema.
- Filesystem and process tools that share one execution world, so a file one
  tool writes is the file the next tool reads.
- Structured errors with a kind, a message and the failing field.

## What it builds

- A planned tool registry and pipeline in `psionic/lib/harness/`, plus
  filesystem and process tools.

## Dependencies

- [Chapter 200](ch200-the-simplest-loop.md): the loop that calls tools.
- [Chapter 53](ch053-choosing-the-next-token.md): constrained decoding for arguments.
- [Chapter 77](ch077-call-a-tool.md): the bounded tool round trip on gpt-oss.

## Done means

- For every registered schema, generated arguments always parse, checked over
  many seeded generations.
- Each pipeline stage can refuse, and the refusal reaches the model as a
  structured error.
- A file written by one tool is visible to a later tool in the same session,
  and not visible outside it.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
