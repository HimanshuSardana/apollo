package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
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
	fmt.Println()

	reader := bufio.NewReader(os.Stdin)

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

		fmt.Println("Thinking...")

		resp, err := sendRequest(client, apiKey, prompt)
		if err != nil {
			fmt.Printf("Error: %v\n\n", err)
			continue
		}

		fmt.Printf("Apollo: %s\n\n", resp)
	}
}

func sendRequest(client *http.Client, apiKey, prompt string) (string, error) {
	type Message struct {
		Role    string `json:"role"`
		Content string `json:"content"`
	}

	type Request struct {
		Model    string    `json:"model"`
		Messages []Message `json:"messages"`
	}

	type Choice struct {
		Message Message `json:"message"`
	}

	type Response struct {
		Choices []Choice `json:"choices"`
	}

	reqBody := Request{
		Model: "minimax-m2.5",
		Messages: []Message{
			{Role: "user", Content: prompt},
		},
	}

	jsonBody, err := json.Marshal(reqBody)
	if err != nil {
		return "", err
	}

	req, err := http.NewRequestWithContext(context.Background(), "POST",
		"https://opencode.ai/zen/go/v1/chat/completions", bytes.NewBuffer(jsonBody))
	if err != nil {
		return "", err
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+apiKey)
	fmt.Printf("API Key: %s", apiKey)

	resp, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("API returned status %d", resp.StatusCode)
	}

	var chatResp Response
	if err := json.NewDecoder(resp.Body).Decode(&chatResp); err != nil {
		return "", err
	}

	if len(chatResp.Choices) == 0 {
		return "", fmt.Errorf("no response from API")
	}

	return chatResp.Choices[0].Message.Content, nil
}

