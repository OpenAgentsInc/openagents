You are taking over a task from a fast explorer agent. The explorer investigated first; what it found is below. Treat it as evidence to check, not as orders.

## The task

I have a sqlite database in /app/trunc.db that was corrupted through binary truncation. Recover as many of the rows as possible, and create a JSON file in /app/recover.json. The output should have the format [{"word": "testwordXY", "value": M}, {"word": "testwordZZ","value": N}, ...]

## Requirements and whether Jev judged them met

- Recover as many of the rows as possible, and create a JSON file in /app/recover.json. (not judged)
- The output should have the format [{"word": "testwordXY", "value": M}, {"word": "testwordZZ","value": N}, ...] (not judged)

## What the explorer concluded

The explorer reached its 0-step bound without a conclusion.

## Files by relevance

### $ list /app (depth 3) (Jev p=0.90)

```
/app:
trunc.db  4096 B
```
### $ read /app/trunc.db (first 200 lines) (Jev p=0.53)

```
[binary file, 4096 bytes]
```

## What to do

Complete the task in the current working directory. Nobody answers questions, so decide from the task and the environment. An automated checker grades the final state of the environment against the task, so verify every requirement, including exact paths, names, and formats, before you stop. The files, command outputs, and setup results in this briefing were gathered just before you started and are complete and current: do not list, read, or run them again. Work in few, large steps: write each file whole in one command, and chain related commands (installs, builds) with && in one call. Before you stop, run the checks the task names and exercise every code path you changed, not only the example the task gives. After a bulk find-and-replace, search the result for occurrences it missed or changed twice. End with a short summary of what you changed and how you checked it.
