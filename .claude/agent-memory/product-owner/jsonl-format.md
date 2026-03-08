# Claude Code JSONL Session Log Format

Verified against live data on 2026-03-08.

## File Location
```
~/.claude/projects/{encoded-project-path}/{session-uuid}.jsonl
```

The encoded project path replaces `/` with `-` and removes the leading
slash. Example:
```
/Volumes/External/Users/fdrake/Source/github.com/fred-drake/claude-seer
->
-Volumes-External-Users-fdrake-Source-github-com-fred-drake-claude-seer
```

## Common Fields (most record types)
- `type`: string - record type
- `uuid`: string - unique record ID
- `parentUuid`: string|null - parent record for threading
- `sessionId`: string - session UUID
- `timestamp`: string - ISO 8601
- `cwd`: string - working directory
- `gitBranch`: string - active git branch
- `version`: string - Claude Code version (e.g. "2.1.71")
- `isSidechain`: bool - whether this is a side conversation
- `userType`: string - "external" for normal usage

## Record Types

### user
- `message.content`: string | array of tool_result objects
- Tool results: `[{"tool_use_id": "...", "type": "tool_result",
  "content": "..."}]`

### assistant
- `message.content[]`: array of content blocks
  - `{type: "text", text: "..."}` - text response
  - `{type: "thinking", thinking: "...", signature: "..."}` -
    extended thinking
  - `{type: "tool_use", id: "...", name: "...", input: {...},
    caller: {...}}` - tool invocation
- `message.usage`: token counts
  - `input_tokens`, `output_tokens`
  - `cache_creation_input_tokens`, `cache_read_input_tokens`
  - `cache_creation.ephemeral_5m_input_tokens`,
    `cache_creation.ephemeral_1h_input_tokens`
- `requestId`: string - API request ID
- `slug`: string - session slug (e.g. "typed-brewing-seahorse")

### progress
- `data.type`: subtype
  - `bash_progress` - shell command output
  - `hook_progress` - hook execution
  - `agent_progress` - subagent activity
  - `search_results_received` - search results
  - `query_update` - query changes
- `toolUseID`, `parentToolUseID`: link to tool calls

### system
- `subtype`: string
  - `stop_hook_summary` - hook execution summary with timings
  - `turn_duration` - turn timing information
- `hookCount`, `hookInfos[]`, `hookErrors[]`: hook details

### file-history-snapshot
- `messageId`: string
- `snapshot`: object - file state at that point
- `isSnapshotUpdate`: bool

### last-prompt
- `lastPrompt`: string - the last user message text
- `sessionId`: string

### queue-operation
- Observed in larger sessions; details TBD

## Tool Names Observed
Bash, Read, Write, Edit, Glob, LSP, ToolSearch, WebFetch,
WebSearch, Agent

## Compaction Detection
No explicit "compaction" or "summary" record type found.
Compaction may need to be inferred from:
- Sudden drop in cache_read_input_tokens between turns
- Large gap in parentUuid chain
- Presence of summarized context in system messages
This needs further investigation with longer sessions.
