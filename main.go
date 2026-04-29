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
	"path/filepath"
	"strings"

	"github.com/charmbracelet/glamour"
	"github.com/chzyer/readline"
	"golang.org/x/term"
	"gopkg.in/yaml.v3"
)

var (
	cwd, _    = os.Getwd()
	debugMode bool
	safeMode  bool
)

type Config struct {
	BaseURL   string `yaml:"base_url"`
	ModelName string `yaml:"model_name"`
}

// AppConfig is the global configuration instance
var AppConfig Config

// usageTracker holds cumulative token usage for a session
var usageTracker = UsageTracker{
	promptTokens:     0,
	completionTokens: 0,
	totalTokens:      0,
	requestCount:     0,
}

// UsageTracker stores token usage statistics
type UsageTracker struct {
	promptTokens     int
	completionTokens int
	totalTokens      int
	requestCount     int
}

type filenameAutoCompleter struct{}

func (c *filenameAutoCompleter) Do(line []rune, pos int) (newLine [][]rune, length int) {
	lineStr := string(line)

	start := pos
	for start > 0 && line[start-1] != ' ' {
		start--
	}

	prefix := lineStr[start:pos]

	dir := filepath.Dir(prefix)
	if dir == "" {
		dir = "."
	}
	base := filepath.Base(prefix)

	// Read directory entries
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, 0
	}

	// Find matching entries
	for _, entry := range entries {
		name := entry.Name()
		if !strings.HasPrefix(name, base) {
			continue
		}

		// Return the suffix that completes the word
		suffix := name[len(base):]
		if entry.IsDir() {
			suffix += "/"
		}

		newLine = append(newLine, []rune(suffix))
	}

	// length=0 means we're inserting at cursor, not replacing
	return newLine, 0
}

// commandAutoCompleter provides tab completion for / commands
type commandAutoCompleter struct{}

var availableCommands = []string{
	"usage",
	"clear",
	"help",
}

// skillAutoCompleter provides tab completion for /skill: command
type skillAutoCompleter struct{}

func (c *skillAutoCompleter) Do(line []rune, pos int) (newLine [][]rune, length int) {
	lineStr := string(line)

	// Only complete if line starts with /skill:
	if !strings.HasPrefix(lineStr, "/skill:") {
		return nil, 0
	}

	// Find the start of the skill name (after /skill:)
	prefix := "/skill:"
	start := len(prefix)
	for start < pos && line[start] == ' ' {
		start++
	}

	// Get the current skill name prefix
	currentPrefix := lineStr[start:pos]

	// Read skills directory
	skillsDir := "/home/himanshu/.agents/skills"
	entries, err := os.ReadDir(skillsDir)
	if err != nil {
		return nil, 0
	}

	// Find matching skills
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		name := entry.Name()
		if !strings.HasPrefix(name, currentPrefix) {
			continue
		}

		suffix := name[len(currentPrefix):]
		newLine = append(newLine, []rune(suffix+" "))
	}

	return newLine, 0
}

func (c *commandAutoCompleter) Do(line []rune, pos int) (newLine [][]rune, length int) {
	lineStr := string(line)

	// Only complete if line starts with /
	if !strings.HasPrefix(lineStr, "/") {
		return nil, 0
	}

	// Find the start of the current word (after the /)
	start := 1 // skip the leading /
	for start < pos && line[start] == ' ' {
		start++
	}

	// Get the current command word (without the /)
	prefixEnd := start
	for prefixEnd < pos && line[prefixEnd] != ' ' {
		prefixEnd++
	}

	prefix := lineStr[start:prefixEnd]

	// Find matching commands
	for _, cmd := range availableCommands {
		if !strings.HasPrefix(cmd, prefix) {
			continue
		}

		suffix := cmd[len(prefix):]
		newLine = append(newLine, []rune(suffix+" "))
	}

	return newLine, 0
}

var SYSTEM_PROMPT = `You are an AI assistant that helps the user understand and navigate the codebase in the current working directory. You have access to the following tools:

- ls [path]: Lists the contents of a directory. Use this to explore the project structure, find files, or see what is in a folder. If no path is provided, it lists the current directory.
- read <path>: Reads the full contents of a file. Use this to examine source code, configuration files, or documentation. The path argument is required.
- bash [cmd]: Executes a shell command and returns the output. Use this for task that requires running commands, such as checking git status, running tests, or using command-line tools.
- edit path=<path> new_content=<content> [old_text=<text>]: Edit a file by providing new content. ALWAYS read the file first, then provide the COMPLETE new content. The user will see a diff preview and can confirm before changes are applied. Use old_text to specify what you're replacing if you want verification.

Guidelines for using tools:
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
	flag.BoolVar(&safeMode, "safe", false, "Ask for confirmation before applying edits")
	flag.Parse()

	// Set safe mode for tools package
	SafeMode = safeMode

	configPath := filepath.Join(cwd, "config.yaml")
	if err := loadConfig(configPath); err != nil {
		fmt.Fprintf(os.Stderr, "Warning: Could not load config.yaml: %v\n", err)
		fmt.Fprintln(os.Stderr, "Using default configuration")
		AppConfig = Config{
			BaseURL:   "https://opencode.ai/zen/go/v1/chat/completions",
			ModelName: "minimax-m2.5",
		}
	}

	if debugMode {
		fmt.Printf("Using API endpoint: %s\n", AppConfig.BaseURL)
		fmt.Printf("Using model: %s\n", AppConfig.ModelName)
	}

	client := &http.Client{}
	apiKey := os.Getenv("OPENCODE_API_KEY")
	if apiKey == "" {
		fmt.Fprintln(os.Stderr, "Error: OPENCODE_API_KEY not set")
		os.Exit(1)
	}

	// Get terminal width for full-width rendering
	termWidth := 120 // default
	if w, _, err := term.GetSize(int(os.Stdout.Fd())); err == nil && w > 0 {
		termWidth = w
	}

	renderer, err := glamour.NewTermRenderer(
		glamour.WithAutoStyle(),
		glamour.WithWordWrap(termWidth),
	)
	if err != nil {
		fmt.Fprintln(os.Stderr, "Error: failed to create markdown renderer:", err)
		os.Exit(1)
	}

	fmt.Println(Bold + Cyan + "Apollo AI Assistant" + Reset)
	fmt.Println(Gray + "Type your prompt and press Enter to send. Type 'quit' to exit." + Reset)
	fmt.Printf(Gray+"Model: %s | Tools: ls, read, bash, edit | Tab: autocomplete\n"+Reset, AppConfig.ModelName)
	fmt.Println(Gray + "Commands: /usage - Show token usage | /skill:<name> - Load a skill (tab complete)" + Reset)
	fmt.Println()

	rl, err := readline.NewEx(&readline.Config{
		Prompt:          Cyan + "You: " + Reset,
		InterruptPrompt: "^C",
		EOFPrompt:       "quit",
		AutoComplete:    &combinedCompleter{},
	})
	if err != nil {
		fmt.Fprintln(os.Stderr, "Error creating readline:", err)
		os.Exit(1)
	}
	defer rl.Close()

	messages := []Message{
		{Role: "system", Content: SYSTEM_PROMPT},
	}

	for {
		fmt.Print(Blue + "You: " + Reset)
		prompt, err := rl.Readline()
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

		// Handle /skill: prefix - load skill and prepend to message
		if strings.HasPrefix(prompt, "/skill:") {
			skillName := strings.TrimSpace(strings.TrimPrefix(prompt, "/skill:"))
			// Extract just the skill name (first word)
			parts := strings.Fields(skillName)
			if len(parts) > 0 {
				skillName = parts[0]
			}
			
			if skillName != "" {
				skillPath := filepath.Join("/home/himanshu/.agents/skills", skillName, "SKILL.md")
				skillContent, err := os.ReadFile(skillPath)
				if err != nil {
					fmt.Printf(Red+"Error: Could not load skill '%s': %v"+Reset+"\n\n", skillName, err)
					continue
				}
				
				// Get the rest of the user's message after the skill name
				restOfMessage := strings.TrimSpace(strings.TrimPrefix(strings.TrimPrefix(prompt, "/skill:"), skillName))
				
				// Prepend skill content to the message
				prompt = string(skillContent) + "\n\n" + restOfMessage
				fmt.Printf(Green+"✓ Loaded skill: %s"+Reset+"\n", skillName)
			}
		}

		if strings.HasPrefix(prompt, "/") {
			parts := strings.Fields(prompt[1:])
			if len(parts) > 0 {
				// Handle /usage command
				if parts[0] == "usage" {
					fmt.Println()
					printUsage()
					fmt.Println()
					continue
				}
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
		var thinkingBuffer strings.Builder
		var lineCount int
		var resp string

		// Tool call chaining loop
		for {
			var thinking string
			var toolCalls []ToolCall
			var currentUsage *Usage
			var err error
			resp, thinking, toolCalls, currentUsage, err = sendRequest(client, apiKey, messages, true, func(chunk string) {
				outputBuffer.WriteString(chunk)
				fmt.Print(chunk)
				lineCount += strings.Count(chunk, "\n")
			}, func(thinkingChunk string) {
				thinkingBuffer.WriteString(thinkingChunk)
				fmt.Print(Dim + thinkingChunk + Reset)
			})
			if err != nil {
				fmt.Printf(Red+"\nError: %v"+Reset+"\n\n", err)
				messages = messages[:len(messages)-1]
				break
			}

			// Update usage tracker
			if currentUsage != nil {
				usageTracker.promptTokens += currentUsage.PromptTokens
				usageTracker.completionTokens += currentUsage.CompletionTokens
				usageTracker.totalTokens += currentUsage.TotalTokens
				usageTracker.requestCount++
			}

			_ = thinking

			// If no tool calls, we're done
			if len(toolCalls) == 0 {
				// Display thinking text
				thinkingStr := thinkingBuffer.String()
				if thinkingStr != "" {
					fmt.Println()
					fmt.Println(Dim + "💭 Thinking: " + thinkingStr + Reset)
				}
				break
			}

			// Print accumulated thinking with dimmed color
			thinkingStr := thinkingBuffer.String()
			if thinkingStr != "" {
				fmt.Println()
				fmt.Println(Dim + "Thinking: " + thinkingStr + Reset)
			}

			// Print all tool call details with dimmed color
			fmt.Println()
			fmt.Printf(Dim+"Tool Calls (%d):"+Reset+"\n", len(toolCalls))

			var toolCallInfos []ToolCallInfo
			for i, tc := range toolCalls {
				fmt.Printf(Dim+"  [%d] Name: %s"+Reset+"\n", i+1, tc.Name)
				fmt.Printf(Dim+"      Arguments: %s"+Reset+"\n", tc.RawArgs)

				arguments := tc.RawArgs
				if arguments == "" {
					argsMap := map[string]string{}
					if len(tc.Args) > 0 {
						argsMap["path"] = tc.Args[0]
					}
					argsBytes, _ := json.Marshal(argsMap)
					arguments = string(argsBytes)
				}
				toolCallInfos = append(toolCallInfos, ToolCallInfo{
					ID:       fmt.Sprintf("call_%d", i+1),
					Type:     "function",
					Function: FC{Name: tc.Name, Arguments: arguments},
				})
			}

			messages = append(messages, Message{
				Role:      "assistant",
				Content:   resp,
				ToolCalls: toolCallInfos,
			})

			// Execute ALL tool calls
			for i, tc := range toolCalls {
				fmt.Printf(Green+"\n▶ Executing [%d/%d]: %s"+Reset+"\n", i+1, len(toolCalls), tc.Name)
				result, err := ExecuteTool(tc.Name, tc.Args)
				if err != nil {
					messages = append(messages, Message{Role: "tool", ToolCallID: fmt.Sprintf("call_%d", i+1), Content: fmt.Sprintf("Error: %v", err)})
				} else {
					messages = append(messages, Message{Role: "tool", ToolCallID: fmt.Sprintf("call_%d", i+1), Content: result})
				}
			}

			// Reset for next iteration (chaining)
			fmt.Print(Red + "\nApollo: " + Reset)
			outputBuffer.Reset()
			thinkingBuffer.Reset()
			lineCount = 0
		}

		messages = append(messages, Message{Role: "assistant", Content: resp})

		// Clear all streamed lines and re-render with glamour
		// Move cursor to beginning of "Apollo:" line, then clear everything below
		fmt.Print("\r") // Go to start of current line
		for i := 0; i < lineCount; i++ {
			fmt.Print("\033[A") // Move up one line
		}
		fmt.Print("\033[J") // Clear from cursor to end of screen

		rendered, renderErr := renderer.Render(outputBuffer.String())
		if renderErr != nil {
			rendered = outputBuffer.String()
		}
		fmt.Printf(Red+"Apollo: "+Reset+"%s"+"\n\n", rendered)
	}
}

func printUsage() {
	if usageTracker.requestCount == 0 {
		fmt.Println(Yellow + "No API requests made yet in this session." + Reset)
		return
	}

	fmt.Println(Bold + "Token Usage (Session)" + Reset)
	fmt.Printf("  %sRequests:%s     %d\n", Cyan, Reset, usageTracker.requestCount)
	fmt.Printf("  %sPrompt Tokens:%s    %s%d%s\n", Cyan, Reset, Green, usageTracker.promptTokens, Reset)
	fmt.Printf("  %sCompletion Tokens:%s %s%d%s\n", Cyan, Reset, Green, usageTracker.completionTokens, Reset)
	fmt.Printf("  %sTotal Tokens:%s     %s%d%s\n", Cyan, Reset, Green, usageTracker.totalTokens, Reset)
}

// Message represents a chat message
type Message struct {
	Role       string         `json:"role"`
	Content    string         `json:"content,omitempty"`
	Thinking   string         `json:"thinking,omitempty"`
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

// Usage holds token usage information from API response
type Usage struct {
	PromptTokens     int `json:"prompt_tokens"`
	CompletionTokens int `json:"completion_tokens"`
	TotalTokens      int `json:"total_tokens"`
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
	Usage   *Usage         `json:"usage,omitempty"`
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
		Thinking  string         `json:"thinking,omitempty"`
		ToolCalls []ToolCallInfo `json:"tool_calls,omitempty"`
	} `json:"message"`
	FinishReason string `json:"finish_reason"`
}

// Response from API
type Response struct {
	Choices []Choice `json:"choices"`
	Usage   *Usage   `json:"usage,omitempty"`
}

func sendRequest(client *http.Client, apiKey string, messages []Message, stream bool, onChunk func(string), onThinking func(string)) (string, string, []ToolCall, *Usage, error) {
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

	editTool := ToolDef{
		Type: "function",
		Function: struct {
			Name        string `json:"name"`
			Description string `json:"description"`
			Parameters  any    `json:"parameters"`
		}{
			Name:        "edit",
			Description: "Edit a file by providing new content. Shows a diff preview before applying changes. Use read first to see current content, then provide the complete new file content.",
			Parameters: map[string]any{
				"type": "object",
				"properties": map[string]any{
					"path": map[string]any{
						"type":        "string",
						"description": "Path to the file to edit",
					},
					"new_content": map[string]any{
						"type":        "string",
						"description": "The complete new content for the file",
					},
					"old_text": map[string]any{
						"type":        "string",
						"description": "Optional: specific text that should be replaced (for verification)",
					},
				},
				"required": []any{"path", "new_content"},
			},
		},
	}

	reqBody := Request{
		Model:    AppConfig.ModelName,
		Messages: messages,
		Tools:    []ToolDef{lsTool, readTool, bashTool, editTool},
		Stream:   stream,
	}

	jsonBody, err := json.Marshal(reqBody)
	if err != nil {
		return "", "", nil, nil, err
	}

	if debugMode {
		fmt.Println(Dim + "Request: " + string(jsonBody) + Reset)
	}

	req, err := http.NewRequestWithContext(context.Background(), "POST",
		AppConfig.BaseURL, bytes.NewBuffer(jsonBody))
	if err != nil {
		return "", "", nil, nil, err
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+apiKey)

	if stream {
		return handleStreamingResponse(client, req, onChunk, onThinking)
	}

	return handleNonStreamingResponse(client, req)
}

func handleStreamingResponse(client *http.Client, req *http.Request, onChunk func(string), onThinking func(string)) (string, string, []ToolCall, *Usage, error) {
	resp, err := client.Do(req)
	if err != nil {
		return "", "", nil, nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", "", nil, nil, fmt.Errorf("API returned status %d", resp.StatusCode)
	}

	var fullContent strings.Builder
	var fullThinking strings.Builder
	var toolCalls []ToolCall
	var accumulatedToolCalls []ToolCallInfo
	var lastUsage *Usage

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

		// Capture usage from stream chunks
		if streamResp.Usage != nil {
			lastUsage = streamResp.Usage
		}

		if len(streamResp.Choices) == 0 {
			continue
		}

		choice := streamResp.Choices[0]
		delta := choice.Delta

		// Accumulate thinking
		if delta.Thinking != "" {
			fullThinking.WriteString(delta.Thinking)
			if onThinking != nil {
				onThinking(delta.Thinking)
			}
		}

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
				// Convert all accumulated tool calls
				for _, tc := range accumulatedToolCalls {
					toolCalls = append(toolCalls, ToolCall{
						Name:    tc.Function.Name,
						Args:    parseToolArgs(tc.Function.Arguments),
						RawArgs: tc.Function.Arguments,
					})
				}
			}
			break
		}
	}

	return fullContent.String(), fullThinking.String(), toolCalls, lastUsage, scanner.Err()
}

func handleNonStreamingResponse(client *http.Client, req *http.Request) (string, string, []ToolCall, *Usage, error) {
	resp, err := client.Do(req)
	if err != nil {
		return "", "", nil, nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", "", nil, nil, fmt.Errorf("API returned status %d", resp.StatusCode)
	}

	var chatResp Response
	if err := json.NewDecoder(resp.Body).Decode(&chatResp); err != nil {
		return "", "", nil, nil, err
	}

	if len(chatResp.Choices) == 0 {
		return "", "", nil, nil, fmt.Errorf("no response from API")
	}

	msg := chatResp.Choices[0].Message

	var toolCalls []ToolCall
	for _, tc := range msg.ToolCalls {
		toolCalls = append(toolCalls, ToolCall{
			Name:    tc.Function.Name,
			Args:    parseToolArgs(tc.Function.Arguments),
			RawArgs: tc.Function.Arguments,
		})
	}

	return msg.Content, msg.Thinking, toolCalls, chatResp.Usage, nil
}

// parseToolArgs extracts arguments from JSON string
func parseToolArgs(args string) []string {
	var argsMap map[string]any
	if err := json.Unmarshal([]byte(args), &argsMap); err != nil {
		return nil
	}

	// Handle edit tool: requires path and new_content
	if path, ok := argsMap["path"].(string); ok {
		if newContent, ok := argsMap["new_content"].(string); ok {
			result := []string{path, newContent}
			if oldText, ok := argsMap["old_text"].(string); ok && oldText != "" {
				result = append(result, oldText)
			}
			return result
		}
		return []string{path}
	}

	if cmd, ok := argsMap["cmd"].(string); ok && cmd != "" {
		return []string{cmd}
	}
	return nil
}

// loadConfig reads configuration from config.yaml
func loadConfig(path string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}

	return yaml.Unmarshal(data, &AppConfig)
}

// combinedCompleter switches between command, skill, and filename completion
type combinedCompleter struct{}

func (c *combinedCompleter) Do(line []rune, pos int) (newLine [][]rune, length int) {
	lineStr := string(line)

	if strings.HasPrefix(lineStr, "/skill:") {
		return (&skillAutoCompleter{}).Do(line, pos)
	}

	if strings.HasPrefix(lineStr, "/") {
		return (&commandAutoCompleter{}).Do(line, pos)
	}

	return (&filenameAutoCompleter{}).Do(line, pos)
}
