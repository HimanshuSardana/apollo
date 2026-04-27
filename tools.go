package main

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
)

// SafeMode controls whether to ask for confirmation before edits
var SafeMode bool

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
			"edit": {
				Name:        "edit",
				Description: "Edit a file by showing diff and applying changes",
				Execute:     executeEdit,
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

	// Validate path - prevent directory traversal
	if strings.Contains(path, "..") {
		return "", fmt.Errorf("path traversal not allowed")
	}

	// Restrict to allowed directories
	allowedPaths := []string{".", "assets", ".."}
	allowed := false
	for _, allowedPath := range allowedPaths {
		if path == allowedPath || strings.HasPrefix(path, allowedPath+"/") {
			allowed = true
			break
		}
	}
	if !allowed {
		return "", fmt.Errorf("access to this path is not allowed")
	}

	cmd := exec.Command("ls", "-la", path)
	out, err := cmd.Output()
	if err != nil {
		if e, ok := err.(*exec.ExitError); ok {
			return "", fmt.Errorf("%s", e.Stderr)
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

	// Validate path - prevent directory traversal
	if strings.Contains(path, "..") {
		return "", fmt.Errorf("path traversal not allowed")
	}

	// Restrict to allowed directories
	allowedPaths := []string{".", "assets", "main.go", "README.md", "Makefile", "tools.go"}
	allowed := false
	for _, allowedPath := range allowedPaths {
		if path == allowedPath || strings.HasPrefix(path, allowedPath+"/") {
			allowed = true
			break
		}
	}
	if !allowed {
		return "", fmt.Errorf("access to this path is not allowed")
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("cannot read %s: %w", path, err)
	}
	return string(data), nil
}

// dangerousCommands is a list of commands that are blocked entirely
var dangerousCommands = []string{
	"rm", "rmdir", "del", "erase", // File deletion
	"mkfs", "mkfs.ext", "mkfs.xfs", // Filesystem creation (destructive)
	"dd",                             // Low-level disk operations
	"wipe", "shred", "secure-delete", // Secure deletion
	"fdisk", "parted", "gparted", // Disk partitioning
	"mount", "umount", "mount.", // Filesystem mounting
	"curl", "wget", "fetch", "rpm", "apt-get", "apt", "yum", "dpkg", // Download/install
	"pip", "npm", "gem", "cargo", "go install", // Package managers
	"python", "python3", "perl", "ruby", "node", "php", "bash", "sh", "zsh", // Interpreters
	"chmod", "chown", "chgrp", // Permission changes
	"chmod +x", "chmod 777", // Dangerous permission escalations
	"sudo", "su", "doas", // Privilege escalation
	"ssh", "scp", "sftp", "nc", "netcat", // Network operations
	"fork",                     // Process forking
	"> /dev/sd", "> /dev/null", // Dangerous redirections
	"eval", "exec", "source", // Shell built-ins
	"alias", "export", "env", // Environment manipulation
}

// dangerousPatterns are patterns that indicate dangerous operations
var dangerousPatterns = []string{
	"| bash", "| sh", "| /bin",
	"& rm", "& del", "& erase",
	"; rm", "; del", "; erase",
	"&& rm", "|| rm",
	"> /", ">> /",
	"2> /",
	"$(", "`", // Command substitution
	"$(",
	"> $HOME", "> ~/.", // Overwriting home
	"wget http", "curl http", // Downloading scripts
	"chmod 777", "chmod +x", // Dangerous permissions
	"sudo -", "su -", // Privilege escalation
	"curl -o", "wget -O", // Download to file
}

// sensitivePaths are paths that should never be accessed
var sensitivePaths = []string{
	"/etc", "/usr", "/bin", "/sbin", "/lib", "/var", "/boot", "/dev",
	"/proc", "/sys", "/root", "/home", "/opt", "/mnt", "/media",
	"/snap", "/lost+found", "/srv",
}

func isCommandBlocked(cmd string) bool {
	cmdLower := strings.ToLower(cmd)
	for _, dangerous := range dangerousCommands {
		if strings.HasPrefix(cmdLower, dangerous) {
			return true
		}
	}
	return false
}

func containsDangerousPattern(cmd string) bool {
	cmdLower := strings.ToLower(cmd)
	for _, pattern := range dangerousPatterns {
		if strings.Contains(cmdLower, pattern) {
			return true
		}
	}
	return false
}

func containsSensitivePath(cmd string) bool {
	cmdLower := strings.ToLower(cmd)
	for _, path := range sensitivePaths {
		if strings.Contains(cmdLower, path) {
			return true
		}
	}
	return false
}

func executeBash(args []string) (string, error) {
	if len(args) == 0 {
		return "", fmt.Errorf("usage: bash <command>")
	}
	command := strings.Join(args, " ")

	// Check for blocked commands
	if isCommandBlocked(command) {
		return "", fmt.Errorf("command '%s' is blocked for security reasons", args[0])
	}

	// Check for dangerous patterns
	if containsDangerousPattern(command) {
		return "", fmt.Errorf("command contains dangerous pattern and is blocked for security reasons")
	}

	// Check for sensitive paths
	if containsSensitivePath(command) {
		return "", fmt.Errorf("command attempts to access sensitive path and is blocked")
	}

	// For commands containing certain keywords, require confirmation
	requiresConfirmation := []string{
		"rm -rf", "rm -r", "rm -f", "rmdir",
		"del ", "erase ",
		"mv ", "move ", // Moving can overwrite
		"cp ", "copy ", // Copy could fill disk
		"> ", ">> ", // Redirection
	}

	needsConfirmation := false
	cmdLower := strings.ToLower(command)
	for _, keyword := range requiresConfirmation {
		if strings.Contains(cmdLower, keyword) {
			needsConfirmation = true
			break
		}
	}

	if needsConfirmation {
		fmt.Printf("WARNING: Potentially dangerous command detected: '%s'\n", command)
		fmt.Printf("Are you sure you want to execute this? (y/N): ")
		var response string
		fmt.Scanln(&response)
		if strings.ToLower(response) != "y" {
			return "", fmt.Errorf("command execution cancelled by user")
		}
	}

	// Final validation - ensure no dangerous substrings
	dangerousSubstrings := []string{
		"../", ".../", "/../",
		"0>/dev/", "1>/dev/", "2>/dev/",
		"~/.bashrc", "~/.profile", "~/.bash_profile",
		"/etc/passwd", "/etc/shadow", "/etc/sudoers",
	}
	for _, sub := range dangerousSubstrings {
		if strings.Contains(command, sub) {
			return "", fmt.Errorf("command contains blocked substring: %s", sub)
		}
	}

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

// generateUnifiedDiff creates a unified diff using the diff command
func generateUnifiedDiff(path string, oldContent, newContent string) (string, error) {
	// Create temp files for diff
	oldFile, err := os.CreateTemp("", "apollo_edit_old_*")
	if err != nil {
		return "", err
	}
	defer os.Remove(oldFile.Name())
	oldFile.WriteString(oldContent)
	oldFile.Close()

	newFile, err := os.CreateTemp("", "apollo_edit_new_*")
	if err != nil {
		return "", err
	}
	defer os.Remove(newFile.Name())
	newFile.WriteString(newContent)
	newFile.Close()

	// Run diff -u for unified format
	cmd := exec.Command("diff", "-u", oldFile.Name(), newFile.Name())
	output, _ := cmd.CombinedOutput()

	// diff returns exit 1 when files differ, which is expected
	result := string(output)

	// Replace temp filenames with actual path
	result = strings.ReplaceAll(result, oldFile.Name(), path)
	result = strings.ReplaceAll(result, newFile.Name(), path)

	return result, nil
}

// colorizeDiff adds ANSI colors to diff output
func colorizeDiff(diff string) string {
	lines := strings.Split(diff, "\n")
	var colored strings.Builder

	for _, line := range lines {
		switch {
		case strings.HasPrefix(line, "---"):
			colored.WriteString("\033[31m" + line + "\033[0m\n") // Red for old file
		case strings.HasPrefix(line, "+++"):
			colored.WriteString("\033[32m" + line + "\033[0m\n") // Green for new file
		case strings.HasPrefix(line, "@@"):
			colored.WriteString("\033[36m" + line + "\033[0m\n") // Cyan for hunk headers
		case strings.HasPrefix(line, "-") && !strings.HasPrefix(line, "---"):
			colored.WriteString("\033[31m" + line + "\033[0m\n") // Red for removed lines
		case strings.HasPrefix(line, "+") && !strings.HasPrefix(line, "+++"):
			colored.WriteString("\033[32m" + line + "\033[0m\n") // Green for added lines
		default:
			colored.WriteString(line + "\n")
		}
	}

	return colored.String()
}

// executeEdit edits a file by showing a diff and applying changes
func executeEdit(args []string) (string, error) {
	if len(args) < 2 {
		return "", fmt.Errorf("usage: edit <path> <new_content> [old_text]")
	}
	path := args[0]
	newContent := args[1]

	// Optional: old text to verify before replacement
	var oldText string
	if len(args) >= 3 {
		oldText = args[2]
	}

	// Validate path
	if strings.Contains(path, "..") {
		return "", fmt.Errorf("path traversal not allowed")
	}

	// Read existing file
	var oldContent string
	if FileExists(path) {
		data, err := os.ReadFile(path)
		if err != nil {
			return "", fmt.Errorf("cannot read %s: %w", path, err)
		}
		oldContent = string(data)
	}

	// If old_text specified, verify it exists in file
	if oldText != "" && !strings.Contains(oldContent, oldText) {
		return "", fmt.Errorf("old_text not found in file - file may have changed")
	}

	// Generate unified diff using diff command
	diffOutput, err := generateUnifiedDiff(path, oldContent, newContent)
	if err != nil {
		// Fallback to simple display if diff command fails
		diffOutput = fmt.Sprintf("--- %s\n+++ %s (modified)\n@@ -0,0 +0,0 @@", path, path)
	}

	// Show diff with colors
	fmt.Println("\n" + strings.Repeat("=", 70))
	fmt.Println("Proposed changes to: " + path)
	fmt.Println(colorizeDiff(diffOutput))

	// In safe mode, ask for confirmation
	if SafeMode {
		fmt.Printf("\nApply these changes? (y/n): ")
		var response string
		fmt.Scanln(&response)

		if strings.ToLower(response) != "y" && strings.ToLower(response) != "yes" {
			return "Changes cancelled", nil
		}
	}

	// Apply the changes
	err = os.WriteFile(path, []byte(newContent), 0o644)
	if err != nil {
		return "", fmt.Errorf("failed to write file: %w", err)
	}

	if SafeMode {
		return fmt.Sprintf("✓ File %s updated successfully", path), nil
	}
	return fmt.Sprintf("✓ File %s updated (use --safe to review changes)", path), nil
}
