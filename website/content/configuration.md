---
title: Configuration
date: 2026-09-13
tags: [reference]
---

Everything apollo reads from its environment, plus the API it talks to.

## Environment

| Variable | Required | Description |
|---|---|---|
| `OPENCODE_API_KEY` | Yes | Your opencode.ai API key |

```sh
export OPENCODE_API_KEY=your_api_key
```

## Flags

| Flag | Description |
|---|---|
| `-d`, `--debug` | Print the raw API request JSON before sending |

```sh
apollo -d
```

Debug mode is the fastest way to understand (or troubleshoot) what the agent is doing — you see exactly what leaves your machine.

## API endpoint

| Setting | Value |
|---|---|
| Model | `minimax-m2.5` |
| URL | `https://opencode.ai/zen/go/v1/chat/completions` |
| Protocol | OpenAI-compatible chat completions |

Model usage is billed through your opencode.ai account, if applicable. Apollo itself is MIT-licensed and free.
