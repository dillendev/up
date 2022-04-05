# Up

## Introduction

Small utility to run one or multiple commands continuously and restart them based on file changes.
Could be used to automatically restart a Go webserver for example.

## Configuration

The `up` command defaults to `up.toml` for configuration which has the following format:

```toml
[service.myapp]
cmd = "go run ./cmd/myapp"
watch = ["**/*.go"]
```