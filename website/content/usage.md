---
title: Using apollo
date: 2026-09-12
tags: [guide, usage]
---

The interactive loop, keybindings and commands.

## The loop

```text
Apollo AI Assistant
Type your prompt and press Enter to send. Type 'quit' to exit.
Available tools: ls, read, bash | Tab: autocomplete filenames

You: _
```

Type a prompt, press Enter, watch it think, execute tools and answer — rendered as formatted Markdown via glamour. Multi-turn context is kept, so follow-ups ("now refactor it") just work.

## Commands

| Input | Behavior |
|---|---|
| Regular prompts | Sent to the model with conversation history |
| `/ls`, `/read …`, `/bash …` | Run a tool directly, no model call |
| `quit` | Exit |

## Tab completion

Filenames autocomplete as you type — handy for `read <Tab>` and `/bash <Tab>` without leaving the prompt.

## Example session

```text
You: list files in current directory
Thinking...
Executing [1/1]: ls
Thinking...
Apollo: Here's what I found...

You: read main.go
Thinking...
Executing [1/1]: read
Thinking...
Apollo: ...

You: quit
```
