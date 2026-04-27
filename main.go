package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"net/http"
	"os"
	"strings"

	"github.com/charmbracelet/glamour"
)

var (
	cwd, _    = os.Getwd()
	debugMode bool
)

var SYSTEM_PROMPT = `You are an AI assistant that helps the user understand and navigate the codebase in the current working directory. You have access to the following tools:

- ls [path]: Lists the contents of a directory. Use this to explore the project structure, find files, or see what is in a folder. If no path is provided, it lists the current directory.
- read <path>: Reads the full contents of a file. Use this to examine source code, configuration files, or documentation. The path argument is required.
- bash [cmd]: Executes a shell command and returns the output. Use this for tasks that require running commands, such as checking git status, running tests, or using command-line tools. If no cmd is provided, it will open an interactive shell session.

Guidelines for using tools:
- Use ls when you need to explore the file system, discover files, or verify a directory's contents before reading.
- Use read when the user asks about specific code, logic, or documentation, and you need to see the file contents to answer accurately.
- You may chain tool calls: list a directory first to find relevant files, then read the ones you need.

Response guidelines:
- Do not output raw file contents you read directly unless the user explicitly asks for them. Instead, summarize, quote, or explain the relevant parts.
- Do not use markdown tables in your responses.
- Keep your responses concise and relevant to the user's request.
- When referencing files, include line numbers where possible, e.g. "src/index.ts:10-20" for lines 10 to 20 in src/index.ts.

Current working directory: ` + cwd + "\n\n"

const (
	Reset = "\033[0m"
	Bold  = "\033[1m"
	Dim   = "\033[2m"

	Black   = "\033[30m"
	Red     = "\033[31m"
	Green   = "\033[32m"
	Yellow  = "\033[33m"
	Blue    = "\033[34m"
	Magenta = "\033[35m"
	Cyan    = "\033[36m"
	White   = "\033[97m"

	Gray = "\033[90m"
)

func main() {
	flag.BoolVar(&debugMode, "d", false, "Print API request JSON for debugging")
	flag.BoolVar(&debugMode, "debug", false, "Print API request JSON for debugging")
	flag.Parse()

	client := &http.Client{}
	apiKey := os.Getenv("OPENCODE_API_KEY")
	if apiKey == "" {
		fmt.Fprintln(os.Stderr, "Error: OPENCODE_API_KEY not set")
		os.Exit(1)
	}

	renderer, err := glamour.NewTermRenderer(glamour.WithAutoStyle())
	if err != nil {
		fmt.Fprintln(os.Stderr, "Error: failed to create markdown renderer:", err)
		os.Exit(1)
	}

	fmt.Println(Bold + Cyan + "Apollo AI Assistant" + Reset)
	fmt.Println(Gray + "Type your prompt and press Enter to send. Type 'quit' to exit." + Reset)
	fmt.Println(Gray + "Available tools: ls, read, bash" + Reset)
	fmt.Println()

	reader := bufio.NewReader(os.Stdin)
	messages := []Message{
		{Role: "system", Content: SYSTEM_PROMPT},
	}

	for {
		fmt.Print(Blue + "You: " + Reset)
		prompt, err := reader.ReadString('\n')
		if err != nil {
			break
		}
		prompt = strings.TrimSpace(prompt)
		if prompt == "" {
			continue
		}
		if strings.ToLower(prompt) == "quit" {
			break
		}

		if strings.HasPrefix(prompt, "/") {
			parts := strings.Fields(prompt[1:])
			if len(parts) > 0 {
				result, err := ExecuteTool(parts[0], parts[1:])
				if err != nil {
					fmt.Printf(Red+"Error: %v"+Reset+"\n\n", err)
				} else {
					fmt.Printf(Green+"Result: %s"+Reset+"\n\n", result)
				}
			}
			continue
		}

		messages = append(messages, Message{Role: "user", Content: prompt})

		fmt.Print(Red + "Apollo: " + Reset)

		var outputBuffer strings.Builder
		resp, toolCall, err := sendRequest(client, apiKey, messages, true, func(chunk string) {
			outputBuffer.WriteString(chunk)
			fmt.Print(chunk)
		})
		if err != nil {
			fmt.Printf(Red+"\nError: %v"+Reset+"\n\n", err)
			messages = messages[:len(messages)-1]
			continue
		}

		if toolCall != nil {
			arguments := toolCall.RawArgs
			if arguments == "" {
				argsMap := map[string]string{}
				if len(toolCall.Args) > 0 {
					argsMap["path"] = toolCall.Args[0]
				}
				argsBytes, _ := json.Marshal(argsMap)
				arguments = string(argsBytes)
			}

			messages = append(messages, Message{
				Role:      "assistant",
				Content:   " ",
				ToolCalls: []ToolCallInfo{{ID: "call_1", Type: "function", Function: FC{Name: toolCall.Name, Arguments: arguments}}},
			})

			fmt.Print(Green + "\nExecuting: " + Reset + toolCall.Name + "\n")
			result, err := ExecuteTool(toolCall.Name, toolCall.Args)
			if err != nil {
				messages = append(messages, Message{Role: "tool", ToolCallID: "call_1", Content: fmt.Sprintf("Error: %v", err)})
			} else {
				messages = append(messages, Message{Role: "tool", ToolCallID: "call_1", Content: result})
			}

			// Get final response
			fmt.Print(Red + "\nApollo: " + Reset)
			outputBuffer.Reset()
			resp, _, err = sendRequest(client, apiKey, messages, true, func(chunk string) {
				outputBuffer.WriteString(chunk)
				fmt.Print(chunk)
			})
			if err != nil {
				fmt.Printf(Red+"\nError: %v"+Reset+"\n\n", err)
				continue
			}
		}

		messages = append(messages, Message{Role: "assistant", Content: resp})

		// Clear the streaming line and re-render with glamour for final output
		fmt.Print("\r\033[K") // Clear to end of line
		rendered, renderErr := renderer.Render(outputBuffer.String())
		if renderErr != nil {
			rendered = outputBuffer.String()
		}
		fmt.Printf(Red+"Apollo: "+Reset+"%s"+"\n\n", rendered)
	}
}

// Message represents a chat message
type Message struct {
	Role       string         `json:"role"`
	Content    string         `json:"content,omitempty"`
	Name       string         `json:"name,omitempty"`
	ToolCallID string         `json:"tool_call_id,omitempty"`
	ToolCalls  []ToolCallInfo `json:"tool_calls,omitempty"`
}

// ToolCallInfo for function calls
type FC struct {
	Name      string `json:"name"`
	Arguments string `json:"arguments"`
}

type ToolCallInfo struct {
	ID       string `json:"id"`
	Type     string `json:"type"`
	Function FC     `json:"function"`
	Index    int    `json:"index,omitempty"`
}

// ToolCall represents a tool invocation
type ToolCall struct {
	Name    string
	Args    []string
	RawArgs string
}

// Request structure for API
type Request struct {
	Model    string    `json:"model"`
	Messages []Message `json:"messages"`
	Tools    []ToolDef `json:"tools,omitempty"`
	Stream   bool      `json:"stream,omitempty"`
}

// StreamChoice represents a streaming chunk
type StreamChoice struct {
	Delta        Message `json:"delta"`
	Index        int     `json:"index"`
	FinishReason *string `json:"finish_reason"`
}

// StreamResponse is a single SSE chunk
type StreamResponse struct {
	Choices []StreamChoice `json:"choices"`
}

// ToolDef is a tool definition for the API
type ToolDef struct {
	Type     string `json:"type"`
	Function struct {
		Name        string `json:"name"`
		Description string `json:"description"`
		Parameters  any    `json:"parameters"`
	} `json:"function"`
}

// Choice from API response
type Choice struct {
	Message struct {
		Role      string         `json:"role"`
		Content   string         `json:"content"`
		ToolCalls []ToolCallInfo `json:"tool_calls,omitempty"`
	} `json:"message"`
	FinishReason string `json:"finish_reason"`
}

// Response from API
type Response struct {
	Choices []Choice `json:"choices"`
}

func sendRequest(client *http.Client, apiKey string, messages []Message, stream bool, onChunk func(string)) (string, *ToolCall, error) {
	lsTool := ToolDef{
		Type: "function",
		Function: struct {
			Name        string `json:"name"`
			Description string `json:"description"`
			Parameters  any    `json:"parameters"`
		}{
			Name:        "ls",
			Description: "List directory contents",
			Parameters: map[string]any{
				"type":       "object",
				"properties": map[string]any{"path": map[string]any{"type": "string"}},
				"required":   []any{},
			},
		},
	}

	readTool := ToolDef{
		Type: "function",
		Function: struct {
			Name        string `json:"name"`
			Description string `json:"description"`
			Parameters  any    `json:"parameters"`
		}{
			Name:        "read",
			Description: "Read file contents",
			Parameters: map[string]any{
				"type":       "object",
				"properties": map[string]any{"path": map[string]any{"type": "string"}},
				"required":   []any{"path"},
			},
		},
	}

	bashTool := ToolDef{
		Type: "function",
		Function: struct {
			Name        string `json:"name"`
			Description string `json:"description"`
			Parameters  any    `json:"parameters"`
		}{
			Name:        "bash",
			Description: "Execute a shell command",
			Parameters: map[string]any{
				"type":       "object",
				"properties": map[string]any{"cmd": map[string]any{"type": "string"}},
				"required":   []any{},
			},
		},
	}

	reqBody := Request{
		Model:    "minimax-m2.5",
		Messages: messages,
		Tools:    []ToolDef{lsTool, readTool, bashTool},
		Stream:   stream,
	}

	jsonBody, err := json.Marshal(reqBody)
	if err != nil {
		return "", nil, err
	}

	if debugMode {
		fmt.Println(Dim + "Request: " + string(jsonBody) + Reset)
	}

	req, err := http.NewRequestWithContext(context.Background(), "POST",
		"https://opencode.ai/zen/go/v1/chat/completions", bytes.NewBuffer(jsonBody))
	if err != nil {
		return "", nil, err
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+apiKey)

	if stream {
		return handleStreamingResponse(client, req, onChunk)
	}

	return handleNonStreamingResponse(client, req)
}

func handleStreamingResponse(client *http.Client, req *http.Request, onChunk func(string)) (string, *ToolCall, error) {
	resp, err := client.Do(req)
	if err != nil {
		return "", nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", nil, fmt.Errorf("API returned status %d", resp.StatusCode)
	}

	var fullContent strings.Builder
	var toolCall *ToolCall
	var accumulatedToolCalls []ToolCallInfo

	scanner := bufio.NewScanner(resp.Body)
	for scanner.Scan() {
		line := scanner.Text()
		if !strings.HasPrefix(line, "data: ") {
			continue
		}

		data := strings.TrimPrefix(line, "data: ")
		if data == "[DONE]" {
			break
		}

		var streamResp StreamResponse
		if err := json.Unmarshal([]byte(data), &streamResp); err != nil {
			continue
		}

		if len(streamResp.Choices) == 0 {
			continue
		}

		choice := streamResp.Choices[0]
		delta := choice.Delta

		// Accumulate content
		if delta.Content != "" {
			fullContent.WriteString(delta.Content)
			if onChunk != nil {
				onChunk(delta.Content)
			}
		}

		// Accumulate tool calls
		if len(delta.ToolCalls) > 0 {
			for _, tc := range delta.ToolCalls {
				if tc.Index < len(accumulatedToolCalls) {
					accumulatedToolCalls[tc.Index].Function.Arguments += tc.Function.Arguments
				} else {
					accumulatedToolCalls = append(accumulatedToolCalls, tc)
				}
			}
		}

		// Check for finish
		if choice.FinishReason != nil {
			if *choice.FinishReason == "tool_calls" && len(accumulatedToolCalls) > 0 {
				tc := accumulatedToolCalls[0]
				toolCall = &ToolCall{
					Name:    tc.Function.Name,
					Args:    parseToolArgs(tc.Function.Arguments),
					RawArgs: tc.Function.Arguments,
				}
			}
			break
		}
	}

	return fullContent.String(), toolCall, scanner.Err()
}

func handleNonStreamingResponse(client *http.Client, req *http.Request) (string, *ToolCall, error) {
	resp, err := client.Do(req)
	if err != nil {
		return "", nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", nil, fmt.Errorf("API returned status %d", resp.StatusCode)
	}

	var chatResp Response
	if err := json.NewDecoder(resp.Body).Decode(&chatResp); err != nil {
		return "", nil, err
	}

	if len(chatResp.Choices) == 0 {
		return "", nil, fmt.Errorf("no response from API")
	}

	msg := chatResp.Choices[0].Message

	var toolCall *ToolCall
	if len(msg.ToolCalls) > 0 {
		tc := msg.ToolCalls[0]
		toolCall = &ToolCall{
			Name:    tc.Function.Name,
			Args:    parseToolArgs(tc.Function.Arguments),
			RawArgs: tc.Function.Arguments,
		}
	}

	return msg.Content, toolCall, nil
}

// parseToolArgs extracts arguments from JSON string
func parseToolArgs(args string) []string {
	var argsMap map[string]any
	if err := json.Unmarshal([]byte(args), &argsMap); err != nil {
		return nil
	}
	if path, ok := argsMap["path"].(string); ok && path != "" {
		return []string{path}
	}
	if cmd, ok := argsMap["cmd"].(string); ok && cmd != "" {
		return []string{cmd}
	}
	return nil
}
