# Apollo AI Agent

A CLI AI coding assistant powered by MiniMax-M2.5 model via opencode.ai API.

## Features

- **CLI Interface** - Interactive terminal UI with ANSI colors
- **Tool Execution** - Execute shell commands from AI requests with automatic chaining
- **Built-in Tools**:
  - `ls` - List directory contents
  - `read` - Read file contents
  - `bash` - Execute shell commands
- **Tab Completion** - Autocomplete filenames in commands
- **Markdown Rendering** - Formatted output using glamour
- **Debug Mode** - Print API request JSON for debugging
- **OpenAI-compatible API** - Uses opencode.ai zenith API

## Setup

1. Set the API key:
   ```bash
   export OPENCODE_API_KEY=your_api_key
   ```

2. Run the agent:
   ```bash
   ./apollo
   # or
   go run .
   ```

3. For debug output:
   ```bash
   ./apollo -d
   # or
   ./apollo --debug
   ```

## Usage

```
Apollo AI Assistant
Type your prompt and press Enter to send. Type 'quit' to exit.
Available tools: ls, read, bash | Tab: autocomplete filenames

You: list files in current directory
Thinking...
Executing [1/1]: ls
Thinking...
Apollo: Here's what I found...

You: read main.go
...

You: quit
```

### Commands

- **Regular prompts** - Send to AI and get response
- **`/ls`** - Manual tool execution (prefix with `/`)
- **`quit`** - Exit

## Configuration

| Env Variable | Description |
|-------------|-------------|
| `OPENCODE_API_KEY` | Your opencode.ai API key |

### API Endpoint

- Model: `minimax-m2.5`
- URL: `https://opencode.ai/zen/go/v1/chat/completions`

## Development

```bash
go build -o apollo .
```

## License

MIT
