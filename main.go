package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"regexp"
	"strings"
)

func main() {
	client := &http.Client{}
	apiKey := os.Getenv("OPENCODE_API_KEY")
	if apiKey == "" {
		fmt.Fprintln(os.Stderr, "Error: OPENCODE_API_KEY not set")
		os.Exit(1)
	}

	fmt.Println("Apollo AI Assistant")
	fmt.Println("Type your prompt and press Enter to send. Type 'quit' to exit.")
	fmt.Println("Available tools: ls")
	fmt.Println()

	reader := bufio.NewReader(os.Stdin)
	messages := []Message{
		{Role: "system", Content: "You are Apollo, an AI coding assistant. Available tools:\n\n- ls: List directory contents. Args: [path]. Use 'ls /path' to list specific directory.\n\nWhen user asks to list files, explore directories, or run shell commands, use the ls tool.\nFormat tool calls as: TOOL:tool_name:args\n\nBe helpful, concise, and focus on writing code."},
	}

	for {
		fmt.Print("You: ")
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

		// Check for manual tool command
		if strings.HasPrefix(prompt, "/") {
			parts := strings.Fields(prompt[1:])
			if len(parts) > 0 {
				result, err := ExecuteTool(parts[0], parts[1:])
				if err != nil {
					fmt.Printf("Error: %v\n\n", err)
				} else {
					fmt.Printf("Result: %s\n\n", result)
				}
			}
			continue
		}

		messages = append(messages, Message{Role: "user", Content: prompt})

		fmt.Println("Thinking...")

		resp, toolCalls, err := sendRequest(client, apiKey, messages)
		if err != nil {
			fmt.Printf("Error: %v\n\n", err)
			messages = messages[:len(messages)-1]
			continue
		}

		// Execute any tool calls
		for _, tc := range toolCalls {
			fmt.Printf("Executing: %s %v\n", tc.Name, tc.Args)
			result, err := ExecuteTool(tc.Name, tc.Args)
			if err != nil {
				messages = append(messages, Message{Role: "tool", Content: fmt.Sprintf("Error: %v", err)})
			} else {
				messages = append(messages, Message{Role: "tool", Content: result})
			}
		}

		// If tool was called, get final response
		if len(toolCalls) > 0 {
			fmt.Println("Thinking...")
			resp, _, err = sendRequest(client, apiKey, messages)
			if err != nil {
				fmt.Printf("Error: %v\n\n", err)
				continue
			}
		}

		messages = append(messages, Message{Role: "assistant", Content: resp})
		fmt.Printf("Apollo: %s\n\n", resp)
	}
}

// Message represents a chat message
type Message struct {
	Role    string `json:"role"`
	Content string `json:"content"`
	Name    string `json:"name,omitempty"`
}

// ToolCall represents a tool invocation
type ToolCall struct {
	Name string
	Args []string
}

// Request structure for OpenAI-compatible API
type Request struct {
	Model    string    `json:"model"`
	Messages []Message `json:"messages"`
	Tools    []ToolDef `json:"tools,omitempty"`
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
	Message Message `json:"message"`
}

// Response from API
type Response struct {
	Choices []Choice `json:"choices"`
}

func sendRequest(client *http.Client, apiKey string, messages []Message) (string, []ToolCall, error) {
	// Define available tools
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
				"type": "object",
				"properties": map[string]any{
					"path": map[string]any{"type": "string"},
				},
				"required": []any{},
			},
		},
	}

	reqBody := Request{
		Model:    "minimax-m2.5",
		Messages: messages,
		Tools:    []ToolDef{lsTool},
	}

	jsonBody, err := json.Marshal(reqBody)
	if err != nil {
		return "", nil, err
	}

	req, err := http.NewRequestWithContext(context.Background(), "POST",
		"https://opencode.ai/zen/go/v1/chat/completions", bytes.NewBuffer(jsonBody))
	if err != nil {
		return "", nil, err
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+apiKey)

	resp, err := client.Do(req)
	if err != nil {
		return "", nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := json.Marshal(messages)
		return "", nil, fmt.Errorf("API returned status %d: %s", resp.StatusCode, string(body))
	}

	var chatResp Response
	if err := json.NewDecoder(resp.Body).Decode(&chatResp); err != nil {
		return "", nil, err
	}

	if len(chatResp.Choices) == 0 {
		return "", nil, fmt.Errorf("no response from API")
	}

	msg := chatResp.Choices[0].Message

	// Parse tool calls from response content
	toolCalls := parseToolCalls(msg.Content)

	// Remove tool call markers from response
	content := removeToolCalls(msg.Content)

	return content, toolCalls, nil
}

// parseToolCalls extracts tool calls from response content
func parseToolCalls(content string) []ToolCall {
	var calls []ToolCall

	// Match TOOL:tool_name:args pattern
	re := regexp.MustCompile(`TOOL:(\w+):(.+)`)
	matches := re.FindAllStringSubmatch(content, -1)

	for _, match := range matches {
		if len(match) == 3 {
			calls = append(calls, ToolCall{
				Name: match[1],
				Args: strings.Fields(match[2]),
			})
		}
	}

	return calls
}

// removeToolCalls removes tool call markers from content
func removeToolCalls(content string) string {
	re := regexp.MustCompile(`TOOL:\w+:.+\n?`)
	return re.ReplaceAllString(content, "")
}