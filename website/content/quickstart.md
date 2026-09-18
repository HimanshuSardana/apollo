---
title: Quickstart
date: 2026-09-10
tags: [guide, install]
---

From zero to your first delegated task in about a minute.

## Install

You need Go 1.25 or newer, then:

```sh
go install github.com/HimanshuSardana/apollo@latest
```

Or build from source:

```sh
git clone https://github.com/HimanshuSardana/apollo && cd apollo
go build -o apollo .
```

One binary, no runtime dependencies.

## Configure

Apollo reasons through MiniMax-M2.5, served via the opencode.ai API. Grab a key and export it:

```sh
export OPENCODE_API_KEY=your_api_key
```

Put that in your shell rc to make it permanent. Details: [Configuration](configuration/).

## Run

```sh
apollo
```

You will see the prompt. Try:

```text
You: list files in current directory
Thinking...
Executing [1/1]: ls
Thinking...
Apollo: Here's what I found...
```

Type `quit` to exit. Tab completes filenames mid-command. Next: [how tools work](tools/).
