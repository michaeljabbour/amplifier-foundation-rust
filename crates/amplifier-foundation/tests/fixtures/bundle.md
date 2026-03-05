---
name: markdown-bundle
version: 1.0.0
description: A bundle defined via markdown frontmatter

providers:
  - module: provider-anthropic
    config:
      model: claude-sonnet-4-20250514

tools:
  - module: tool-filesystem
  - module: tool-bash

session:
  orchestrator:
    module: loop-basic
---

# Agent Instructions

You are a helpful coding assistant.

## Guidelines

- Write clean, well-documented code
- Follow established patterns in the codebase
- Run tests before committing changes

## Constraints

- Do not modify files outside the project directory
- Always explain your reasoning before making changes
