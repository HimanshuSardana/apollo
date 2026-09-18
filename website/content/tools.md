---
title: Built-in tools
date: 2026-09-11
tags: [guide, tools]
---

Apollo ships with three tools. The agent calls them on its own — and you can call them yourself.

## The tools

| Tool | What it does |
|---|---|
| `ls` | List directory contents, so the agent sees your project layout |
| `read` | Read file contents into context before answering |
| `bash` | Execute shell commands |

When the agent needs ground truth, it requests a tool call. Apollo executes it locally, feeds the result back into the conversation, and the agent keeps going — chaining calls automatically until your question is answered:

```text
You: read main.go
Thinking...
Executing [1/1]: read
Thinking...
Apollo: Here's what main.go does...
```

## Manual execution

Prefix any tool with `/` to run it yourself without involving the model:

```text
You: /ls
main.go  go.mod  README.md
```

Useful for pulling context into the conversation before you ask the real question.

## Safety

Tools execute real shell commands on your machine — that is the point, and also the reason to read what the agent plans to run. When in doubt, run [debug mode](configuration/) to see the full request/response cycle.
