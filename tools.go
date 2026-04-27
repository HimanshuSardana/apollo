package main

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
)

// ExecTool represents an executable shell tool
type ExecTool struct {
	Name        string
	Description string
	Execute     func(args []string) (string, error)
}

// CmdTools holds all available executables
type CmdTools struct {
	tools map[string]ExecTool
}

func NewCmdTools() *CmdTools {
	return &CmdTools{
		tools: map[string]ExecTool{
			"ls": {
				Name:        "ls",
				Description: "List directory contents",
				Execute:     executeLS,
			},
			"read": {
				Name:        "read",
				Description: "Read file contents",
				Execute:     executeRead,
			},
			"bash": {
				Name:        "bash",
				Description: "Execute a shell command",
				Execute:     executeBash,
			},
		},
	}
}

func (t *CmdTools) Get(name string) (ExecTool, bool) {
	tool, ok := t.tools[name]
	return tool, ok
}

func executeLS(args []string) (string, error) {
	path := "."
	if len(args) > 0 {
		path = strings.Join(args, " ")
	}

	cmd := exec.Command("ls", "-la", path)
	out, err := cmd.Output()
	if err != nil {
		if e, ok := err.(*exec.ExitError); ok {
			return "", fmt.Errorf(string(e.Stderr))
		}
		return "", err
	}
	return string(out), nil
}

func executeRead(args []string) (string, error) {
	if len(args) == 0 {
		return "", fmt.Errorf("usage: read <path>")
	}
	path := args[0]

	data, err := os.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("cannot read %s: %w", path, err)
	}
	return string(data), nil
}

func executeBash(args []string) (string, error) {
	if len(args) == 0 {
		return "", fmt.Errorf("usage: bash <command>")
	}
	command := strings.Join(args, " ")
	cmd := exec.Command("bash", "-c", command)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("%w: %s", err, string(out))
	}
	return string(out), nil
}

// ExecuteTool runs a tool by name with optional arguments
func ExecuteTool(name string, args []string) (string, error) {
	tools := NewCmdTools()
	tool, ok := tools.Get(name)
	if !ok {
		return "", fmt.Errorf("tool not found: %s", name)
	}
	return tool.Execute(args)
}

// FileExists checks if a file or directory exists
func FileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

