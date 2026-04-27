.PHONY: run build build-release

# Default command
run: 
	go run .

# Build the project
build: 
	go build -o apollo .

# Build with release flags and upx compression
build-release:
	go build -ldflags="-s -w" -o apollo .
	upx --best --lzma apollo
