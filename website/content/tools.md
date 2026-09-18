---
title: Built-in tools
date: 2026-09-11
tags: [guide, tools]
---

Apollo ships with four tools. The agent calls them on its own — and you can call them yourself.

## The tools

| Tool | What it does |
|---|---|
| `ls` | List directory contents, so the agent sees your project layout |
| `read` | Read file contents into context before answering |
| `bash` | Execute shell commands |
| `edit` | Edit a file — shows a colorized unified diff before applying |

When the agent needs ground truth, it requests a tool call. Apollo executes it locally, feeds the result back into the conversation, and the agent keeps going — chaining calls automatically until your question is answered:

```text
You: read main.go
Thinking...
Executing [1/1]: read
Thinking...
Apollo: Here's what main.go does...
```

## Diff previews

`edit` never rewrites silently. Before applying a change it prints the unified diff (`diff -u` output, colorized), so you see exactly what moves:

```text
You: fix the off-by-one in main.go
Thinking...
Executing [1/1]: edit
--- main.go
+++ main.go (modified)
@@ -40,7 +40,7 @@
- for i := 0; i <= n; i++ {
+ for i := 0; i < n; i++ {
Thinking...
Apollo: Fixed — the loop ran one past the end...
```

## Manual execution

Prefix any tool with `/` to run it yourself without involving the model:

```text
You: /ls
main.go  go.mod  README.md
```

Useful for pulling context into the conversation before you ask the real question.

## Skills

For reusable workflows, load an agent skill with `/skill:<name>` — see [Skills](skills/).

## Safety

Tools execute real shell commands on your machine — that is the point, and also the reason to read what the agent plans to run. When in doubt, run [debug mode](configuration/) to see the full request/response cycle.
