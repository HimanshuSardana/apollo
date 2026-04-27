# Git Learning Notes

## Command 1: git config
Sets up your identity for git to use in commits.
```bash
git config --global user.name "Your Name"
git config --global user.email "your@email.com"
```
- `--global` applies to all repositories on your machine
- Without `--global`, it only applies to the current repository

## Command 2: git init
Initializes a new Git repository in the current directory.
```bash
git init
```
- Creates a `.git` folder
- Makes the directory a Git project
- Only run once per project
