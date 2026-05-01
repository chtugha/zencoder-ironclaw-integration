# Technical Specification
## Zencoder Native Skill Override System

**Version**: 1.0  
**Based on**: `requirements.md` v1.0  
**IronClaw version**: 0.27.0 (main branch, verified)  
**Status**: Draft

---

## Technical Context

### Language and Format

All deliverables are plain-text **SKILL.md** files — YAML frontmatter delimited by `---` followed by a markdown body. No Rust, TypeScript, or other compiled code is involved. The files are consumed by IronClaw's skill registry at agent startup.

### Verified Source Constants (from `crates/ironclaw_skills/src/`)

| Constant | Value | Source |
|---|---|---|
| `MAX_KEYWORDS_PER_SKILL` | 20 | `types.rs` |
| `MAX_PATTERNS_PER_SKILL` | 5 | `types.rs` |
| `MAX_TAGS_PER_SKILL` | 10 | `types.rs` |
| `MIN_KEYWORD_TAG_LENGTH` | 3 chars | `types.rs` |
| `MAX_PROMPT_FILE_SIZE` | 65,536 bytes (64 KiB) | `types.rs` |
| `MAX_REQUIRED_SKILLS_PER_MANIFEST` | 10 | `types.rs` |
| `MAX_DISCOVERED_SKILLS` | 100 | `registry.rs` |
| `DEFAULT_MAX_SCAN_DEPTH` | 3 | `registry.rs` |
| `SKILL_NAME_PATTERN` | `^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$` | `validation.rs` |
| `SKILL_VERSION_PATTERN` | `^[a-zA-Z0-9._\-+~]{1,32}$` | `validation.rs` |
| Token budget rejection | `(body_bytes × 0.25) > (declared × 2)` → rejected | `registry.rs` |

### Multi-Word Keyword Scoring Behavior (Verified from `selector.rs`)

`MIN_KEYWORD_TAG_LENGTH` is checked against the `.len()` of the entire keyword string (byte length), **not** against individual whitespace-separated words. A multi-word keyword like `"delegate to zencoder"` (20 bytes) passes the 3-char minimum filter.

Scoring operates in two tiers:
- **Exact word match (10 pts)**: `message.split_whitespace().any(|word| word == keyword)` — compares each message word to the entire keyword string. Multi-word keywords **never** match this tier because no single message word equals a multi-word string.
- **Substring match (5 pts)**: `message.contains(keyword)` — matches if the keyword appears as a contiguous phrase in the message. Multi-word keywords **do** match this tier when the phrase appears verbatim.

**Implication**: Multi-word keywords are safe to use but score at most 5 pts (substring) per hit, never 10 pts (exact word). This is acceptable for activation — the keywords still contribute to score alongside pattern and tag matches.

### Token Budget Formula

```
max_body_bytes = declared_max_context_tokens × 8
```

The registry computes `approx_tokens = body_bytes × 0.25` and rejects the skill when `approx_tokens > declared × 2`, which rearranges to `body_bytes > declared × 8`.

### Discovery Order (earlier source wins on name collision)

1. Workspace: `<workspace>/skills/` — `SkillTrust::Trusted`
2. User: `~/.ironclaw/skills/` — `SkillTrust::Trusted`
3. Installed: `~/.ironclaw/installed_skills/` — `SkillTrust::Installed`
4. Bundled (compiled into binary) — `SkillTrust::Trusted`

Placing `skills/<name>/SKILL.md` in the repository (workspace path) or copying it to `~/.ironclaw/skills/<name>/SKILL.md` (user path) fully replaces the bundled skill of the same `name:`.

### Gating Behavior

- `requires.bins`, `requires.env`, `requires.config`: **blocking** — skill silently skipped if any requirement is absent.
- `requires.skills`: **advisory only** — skill loads regardless; a `WARN` log is emitted per missing companion.

### Existing Repository Layout

```
skills/
  zencoder.SKILL.md   ← existing monolith (18 KB, flat-layout)
zencoder-tool/        ← WASM tool (no changes in scope)
wit/tool.wit          ← WIT interface (no changes in scope)
```

---

## Implementation Approach

### Mechanism

Six new `SKILL.md` files are placed in subdirectory layout under `skills/`. Five override IronClaw's bundled skills by declaring the identical `name:` field. One (`zencoder`) is a new slim core that supersedes the existing monolith.

The existing `skills/zencoder.SKILL.md` (flat-layout monolith) **must be removed** as part of the implementation. When both `skills/zencoder.SKILL.md` (flat) and `skills/zencoder/SKILL.md` (subdirectory) exist in the same workspace, both declare `name: zencoder` and IronClaw's `discover_from_dir` scan order within a single directory is OS-dependent (filesystem iteration order). This creates immediately undefined behavior for anyone using this repository as a workspace — whichever file is scanned first wins. The flat file must be deleted (not just renamed) so that only `skills/zencoder/SKILL.md` is discovered. A git history preserves the original content for reference.

### Prompt Body Structure (All Six Files)

Every replacement SKILL.md body follows this layout:

```
## Zencoder Routing Layer

[Minimal state classification — 3 bullets]
[Per-skill routing rules — condition → action pairs]

---

<!-- upstream: <name> v<version> nearai/ironclaw/skills/<name>/SKILL.md @main -->

[Original native skill content, verbatim or condensed]
```

**Section ordering rationale**: The routing layer appears before native content so the model reads it first on every activation, regardless of the token allocation cutoff. If the skill prompt is truncated, native content is trimmed from the end — the routing layer is preserved.

### Minimal State Classification Rule (Self-Contained)

Each replacement skill's routing layer includes these three lines verbatim (no cross-references):

```
- healthy: last zencoder-tool call this conversation returned HTTP 2xx, or no call yet (default).
- degraded: last zencoder-tool call returned 401, 402, 429, 5xx, or a network/timeout error.
- unavailable: zencoder-tool returned a "tool not found" / "not registered" error (permanent this session).
```

When `degraded` or `unavailable`: skip Zencoder routing entirely and fall through to native behavior. Do not duplicate the full resilience state machine (retry logic, backoff, recovery probing) — that lives exclusively in `skills/zencoder/SKILL.md`.

### Native Content Handling

- **Verbatim** (coding, commit, delegation): body fits well within the budget ceiling after adding the routing layer. Original content is included unchanged.
- **Condensed** (code-review, plan-mode): original body exceeds the target of ≤ 1,500 tokens (≤ 6,000 bytes) for content efficiency. Condensation preserves all behavioral rules and decision logic while trimming verbose examples, repeated preamble text, and non-essential edge-case prose. The condensed body must produce functionally identical model behavior on identical inputs.

### Version Tracking Comment

Each file includes an HTML comment immediately before the original content block:

```
<!-- upstream: <skill-name> v<version> nearai/ironclaw/skills/<skill-name>/SKILL.md @main -->
```

This comment survives both YAML parsing and IronClaw's frontmatter round-trip because it lives in the markdown body (not the YAML block). Maintainers updating an upstream version swap only the text within this comment and the content block that follows it.

---

## Source Code Structure Changes

```
skills/
  zencoder.SKILL.md              ← REMOVED (flat-layout monolith, replaced by zencoder/SKILL.md)
  coding/
    SKILL.md                     ← NEW: overrides bundled "coding" v1.0.0
  code-review/
    SKILL.md                     ← NEW: overrides bundled "code-review" v2.0.0
  plan-mode/
    SKILL.md                     ← NEW: overrides bundled "plan-mode" v0.1.0
  commit/
    SKILL.md                     ← NEW: overrides bundled "commit" v1.0.0
  delegation/
    SKILL.md                     ← NEW: overrides bundled "delegation" v0.1.0
  zencoder/
    SKILL.md                     ← NEW: slim Zencoder core (replaces monolith)
```

One file deleted: `skills/zencoder.SKILL.md`. No changes to `zencoder-tool/` (Rust WASM), `wit/tool.wit`, scripts, or any other file.

---

## Per-File Technical Contract

### `skills/coding/SKILL.md`

**Frontmatter** — must match exactly:

```yaml
name: coding
version: "1.0.0+zencoder.1"
description: Best practices for code editing, search, and file operations
activation:
  keywords:
    - "code"
    - "edit"
    - "fix"
    - "implement"
    - "refactor"
    - "bug"
    - "function"
    - "class"
    - "file"
    - "module"
    - "test"
    - "compile"
    - "build"
    - "error"
    - "change"
    - "rename"
    - "delete"
    - "add"
    - "update"
  exclude_keywords:
    - "memory"
    - "routine"
    - "schedule"
  patterns:
    - "(?i)(add|remove|update|modify|create|delete|rename|move)\\s.*(file|function|class|method|variable|import)"
    - "(?i)(fix|debug|investigate|trace|find)\\s.*(bug|error|issue|crash|fail)"
  tags:
    - "development"
    - "coding"
  max_context_tokens: 1500
```

**Keyword/pattern counts**: 19 keywords (cap: 20), 3 exclude keywords (cap: 20), 2 patterns (cap: 5), 2 tags (cap: 10). All within limits. ✓

**Token budget**: `max_context_tokens: 1500` → max body = 12,000 bytes. Target content: ≤ 2,000 bytes total body (well within the 12,000-byte ceiling).

**Routing layer behavior**:

1. If a `task_id` from a prior `zencoder-tool` response is present in this conversation AND state is `healthy`:
   - Call `check_solution_status` before reaching for local edit primitives.
   - Only use `apply_patch` / `read_file` / `write_file` when the user explicitly says "do it yourself" or when `check_solution_status` confirms no active remote task covers this work.
2. If no `task_id` is in scope AND the user explicitly asks to delegate, hand off, or send a coding problem to Zencoder AND state is `healthy`:
   - Call `solve_coding_problem` as the delegation path. If no `project_id` is known, call `list_projects` first.
   - Do NOT proactively offer delegation on every coding request — only when the user's message signals intent to delegate (e.g., "delegate this", "have zencoder fix this", "send this to zenflow"). Generic requests like "fix the bug" or "add a test" must fall through to native local editing without any Zencoder prompt.
3. All other cases: fall through to native behavior without any Zencoder call.

**Native content**: verbatim copy of upstream `coding` v1.0.0 body.

---

### `skills/code-review/SKILL.md`

**Frontmatter** — must match exactly:

```yaml
name: code-review
version: "2.0.0+zencoder.1"
description: Paranoid architect review of code changes for bugs, security, missing tests, and undocumented assumptions. Works on local git diffs OR a GitHub pull request (e.g. `owner/repo N`). For PRs, can post findings as line-level review comments.
activation:
  keywords:
    - "review"
    - "code review"
    - "review changes"
  patterns:
    - "(?i)review\\s.*(code|changes|diff|PR|pull request|commit)"
    - "(?i)(check|look at|inspect)\\s.*(changes|diff|code)"
    - "(?i)review\\s+[a-z0-9._-]+/[a-z0-9._-]+\\s+#?\\d+"
  tags:
    - "code-review"
    - "quality"
    - "security"
  max_context_tokens: 2500
requires:
  skills:
    - github
```

**Keyword/pattern counts**: 3 keywords (cap: 20), 3 patterns (cap: 5), 3 tags (cap: 10). Within limits. ✓  
**`requires.skills: [github]`**: preserved from upstream; advisory only, does not block loading. ✓

**Token budget**: `max_context_tokens: 2500` → max body = 20,000 bytes. Target content: ≤ 6,000 bytes total body (requiring condensation of original ~8,200-byte body).

**Routing layer behavior**:

1. If a `task_id` from a prior `zencoder-tool` response is present in this conversation AND the review subject relates to that task (the user references the task, or the branch/PR under review was mentioned in a prior `check_solution_status` or `solve_coding_problem` response) AND state is `healthy`:
   - After completing the review, attach findings via `update_task`. **`update_task` issues a PATCH that fully replaces the `description` field** (verified in `zencoder-tool/src/lib.rs` `handle_update_task`). The model must first call `get_task` to read the current description, then pass `description = <existing text> + "\n\n## Code Review Findings\n" + <findings>` to `update_task`. Do not pass only the findings text — that would discard the existing description.
2. If the user explicitly asks to track a PR for ongoing review AND state is `healthy`:
   - Suggest creating a `create_automation` for ongoing PR tracking instead of a one-off review.
3. All other cases: fall through to original review behavior (local path, GitHub PR path).

**Native content**: condensed from upstream `code-review` v2.0.0. Condensation rules:
- Retain: six-lens review structure (3a–3f), severity scale, findings table format, GitHub PR API calls (exact code).
- Remove or shorten: Step 1 preamble prose (keep only the essential async pattern note and code block), repetitive rule prose in Step 5 (keep code block + format rule), and the verbose "if the PR touches more than 20 files" paragraph (reduce to a single bullet).
- Maximum condensed body size: 5,000 bytes (leaving ~1,000 bytes for the routing layer).

---

### `skills/plan-mode/SKILL.md`

**Frontmatter** — must match exactly:

```yaml
name: plan-mode
version: "0.1.0+zencoder.1"
description: Structured planning mode for autonomous task execution. Creates plans as MemoryDocs, executes via Missions, tracks progress with live checklist.
activation:
  keywords:
    - "[PLAN MODE]"
    - plan mode
    - create a plan
    - make a plan
    - execution plan
    - step by step plan
  patterns:
    - "\\[PLAN MODE\\]"
    - "plan (out|how to|before|for)"
  tags:
    - planning
    - autonomous
    - task-management
  max_context_tokens: 2500
```

**Keyword/pattern counts**: 6 keywords (cap: 20), 2 patterns (cap: 5), 3 tags (cap: 10). Within limits. ✓  
Note: `"[PLAN MODE]"` is a quoted YAML string (11 chars); square brackets must be quoted to avoid YAML flow-sequence parsing.

**Token budget**: `max_context_tokens: 2500` → max body = 20,000 bytes. Target content: ≤ 6,000 bytes total body (original is ~2,900 bytes, so no condensation required; budget is available for the routing layer).

**Routing layer behavior**:

1. If a `task_id` from a prior `zencoder-tool` response is present in this conversation AND state is `healthy`:
   - Use `get_plan`, `update_plan_step`, and `add_plan_steps` as the canonical plan store. (All three verified as `ZencoderAction` variants in `zencoder-tool/src/lib.rs`; `create_plan` also available for new plans.)
   - Do NOT create a parallel local memory-doc plan (`memory_write` to `plans/<slug>.md`).
   - `get_plan` requires `project_id` + `task_id`; obtain `project_id` from prior context or call `list_projects`.
2. If no `task_id` is in scope: fall through to original `plan-mode` behavior (memory-doc plans, `plan_update`, `mission_create`/`mission_fire`).

**Native content**: verbatim copy of upstream `plan-mode` v0.1.0 body.

---

### `skills/commit/SKILL.md`

**Frontmatter** — must match exactly:

```yaml
name: commit
version: "1.0.0+zencoder.1"
description: Generate git commit messages from staged changes
activation:
  keywords:
    - "commit"
    - "git commit"
  patterns:
    - "(?i)(create|make|write|generate)\\s.*commit"
    - "(?i)commit\\s.*(message|changes|staged)"
  tags:
    - "git"
    - "version-control"
  max_context_tokens: 1000
```

**Keyword/pattern counts**: 2 keywords (cap: 20), 2 patterns (cap: 5), 2 tags (cap: 10). Within limits. ✓

**Token budget**: `max_context_tokens: 1000` → max body = 8,000 bytes. Target content: ≤ 4,000 bytes total body. Original body is ~810 bytes; budget is ample.

**Routing layer behavior**:

1. Check conversation context for the most recently mentioned Zencoder task status.
2. If a `task_id` is in scope AND that task's last known status is `inprogress` or `inreview` AND state is `healthy`:
   - Do NOT commit locally. The remote agent owns the branch.
   - Warn the user once: suggest calling `check_solution_status` to verify before committing.
   - If the user explicitly confirms they want to commit anyway, proceed with native behavior.
3. If the task is `done`, `cancelled`, or no `task_id` is in scope: fall through to native commit behavior.

**Native content**: verbatim copy of upstream `commit` v1.0.0 body.

---

### `skills/delegation/SKILL.md`

**Frontmatter** — must match exactly:

```yaml
name: delegation
version: "0.1.0+zencoder.1"
description: Helps users delegate tasks, break them into steps, set deadlines, and track progress via routines and memory.
activation:
  keywords:
    - delegate
    - hand off
    - assign task
    - help me with
    - take care of
    - remind me to
    - schedule
    - plan my
    - manage my
    - track this
  patterns:
    - "can you.*handle"
    - "I need (help|someone) to"
    - "take over"
    - "set up a reminder"
    - "follow up on"
  tags:
    - personal-assistant
    - task-management
    - delegation
  max_context_tokens: 1500
```

**Keyword/pattern counts**: 10 keywords (cap: 20), 5 patterns (cap: 5 — AT THE LIMIT), 3 tags (cap: 10). Within limits. ✓  
**Critical**: No additional patterns can be added without dropping one. Do not add Zencoder patterns.

**Token budget**: `max_context_tokens: 1500` → max body = 12,000 bytes. Target content: ≤ 6,000 bytes total body. Original body is ~1,100 bytes; budget is ample.

**Routing layer behavior**:

1. Classify the delegation request type using the following heuristic:
   - **Coding delegation**: request mentions any of — code, file, function, method, class, API, test, build, compile, refactor, bug, error, repository, PR, branch, implement, fix, feature, deploy.
   - **Non-coding delegation**: calendar, reminder, schedule, research, document, email, meeting, follow-up, track, agenda.
2. If coding delegation AND state is `healthy`:
   - Call `solve_coding_problem` as the primary path. Do NOT use `routine_create` or `memory_write` to `tasks/<name>.md`.
   - If no `project_id` is known, call `list_projects` first and ask the user to pick one.
3. If non-coding delegation OR state is `degraded`/`unavailable`: fall through to original delegation behavior (`memory_write`, `routine_create`).

**Native content**: verbatim copy of upstream `delegation` v0.1.0 body.

---

### `skills/zencoder/SKILL.md` (Slim Core)

**Frontmatter**:

```yaml
name: zencoder
version: "1.3.0+slim.1"
description: Routes Zencoder/Zenflow operations through the zencoder-tool WASM extension. Slim core — routing logic for coding, review, plan, commit, and delegation is embedded in the respective replacement skills.
activation:
  keywords:
    - "zencoder"
    - "zenflow"
    - "zencoder-tool"
    - "solve_coding_problem"
    - "check_solution_status"
    - "list_projects"
    - "create_task"
    - "list_tasks"
    - "update_task"
    - "get_plan"
    - "update_plan_step"
    - "create_automation"
    - "delegate to zencoder"
    - "delegate to zenflow"
    - "zencoder task"
    - "zenflow task"
    - "zencoder project"
    - "zenflow project"
    - "api.zencoder.ai"
    - "auth.zencoder.ai"
  patterns:
    - "(?i)zen(coder|flow)"
    - "(?i)(delegate|hand[- ]off|send|push|forward)\\s.*(to|on)\\s+zen(coder|flow)"
    - "(?i)(have|let|ask|tell|get)\\s+zen(coder|flow)"
    - "(?i)solve[_ ]coding[_ ]problem"
    - "(?i)zen(coder|flow)\\s+(project|task|plan|workflow|automation|branch|status)"
  tags:
    - "zencoder"
    - "zenflow"
    - "delegation"
    - "coding"
    - "task-management"
    - "automation"
  max_context_tokens: 1000
requires:
  skills:
    - coding
    - code-review
    - plan-mode
    - commit
    - delegation
```

**Keyword/pattern counts**: 20 keywords (cap: 20 — AT THE LIMIT), 5 patterns (cap: 5 — AT THE LIMIT), 6 tags (cap: 10). Within limits. ✓  
**`requires.skills`**: 5 companions (cap: 10). Advisory only — skill loads regardless. IronClaw emits a `WARN` log per missing companion. ✓

**Token budget**: `max_context_tokens: 1000` → max body = 8,000 bytes. Target body: ≤ 4,000 bytes.

**Body content** (in priority order if space is tight):

1. Tool overview: 17 actions in 6 categories (one-line each).
2. Authentication: PAT generation URL, helper script command, `ironclaw tool auth zencoder-tool` command, rotation on expiry.
3. `solve_coding_problem` decision flow: 3-step numbered list.
4. `check_solution_status` decision flow: 3-step numbered list.
5. Task management: `list_tasks` (status filter), `update_task` (requires `project_id` + `task_id`; at least one of `title`/`description`/`status` must be non-empty), `get_plan` (requires `project_id` + `task_id`).
6. Automation management: `create_automation` (name + schedule format), `toggle_automation`.
7. Input constraints: UUID format, status case rules (task lowercase / step PascalCase), 64 KiB text cap.
8. Error handling: 6-row table (401, 402, 429, 5xx, network, unavailable) with per-row action.
9. Resilience state machine: condensed to a 3-state transition table + lazy probe conditions. Full machine is here; replacement skills contain only the 3-bullet minimal classifier.
10. Anti-patterns: 4 bullet points (no raw HTTP, no credential in chat, no create_task + start for coding, no mirroring plan state).

**Content omitted vs. monolith**:
- Verbose per-skill composition rules (now embedded in replacement skills) → replaced by one cross-reference sentence.
- Exhaustive fallback table per Zencoder action → compressed to essential entries or omitted.
- Reconciliation prose for `pending_zencoder_sync` → omitted from slim core (referenced in full in replacement skill routing layers where applicable).

---

## Token Budget Compliance Table

| Skill | `max_context_tokens` | Max allowed body bytes | Bundled body (bytes, approx) | Zencoder routing layer (bytes, approx) | Target total body (bytes) | Status |
|---|---|---|---|---|---|---|
| `coding` | 1,500 | 12,000 | ~1,090 | ~500 | ≤ 2,000 | Well within |
| `code-review` | 2,500 | 20,000 | ~8,200 | ~500 | ≤ 6,000 (condensed) | Requires condensation |
| `plan-mode` | 2,500 | 20,000 | ~2,900 | ~500 | ≤ 4,000 | Within |
| `commit` | 1,000 | 8,000 | ~810 | ~400 | ≤ 1,500 | Well within |
| `delegation` | 1,500 | 12,000 | ~1,100 | ~500 | ≤ 2,000 | Well within |
| `zencoder` (slim) | 1,000 | 8,000 | N/A (new) | N/A | ≤ 4,000 | Must condense monolith |

The byte count check operates on the **UTF-8 body bytes** (after frontmatter is stripped), not on the full file. The frontmatter contributes to `MAX_PROMPT_FILE_SIZE` (64 KiB total) but not to the token budget check.

---

## Data Model / API / Interface Changes

No changes to the `zencoder-tool` WASM binary, the WIT interface, the capabilities JSON, or the Zencoder API endpoints. Six new SKILL.md files are added; one existing file (`skills/zencoder.SKILL.md`) is deleted.

### Context Variables (Model-Tracked, Not Injected)

The routing layer relies on conversational state the model tracks internally from prior tool call results:

| Variable | Type | Source | Default |
|---|---|---|---|
| `task_id` | UUID string | Field in `solve_coding_problem` / `create_task` / `check_solution_status` response | absent (no routing) |
| `project_id` | UUID string | Field in `list_projects` / `solve_coding_problem` response | absent |
| `zencoder_state` | enum (healthy/degraded/unavailable) | Classified from last tool call HTTP status | `healthy` |
| `last_task_status` | string | Field in `check_solution_status` / `get_task` response | absent |

These are not IronClaw context injections; they are instructed tracking patterns in the skill prompt.

### `requires.skills` Companion Declarations (Advisory)

The slim `zencoder` core declares all five replacement skills as companions:

```
coding, code-review, plan-mode, commit, delegation
```

IronClaw logs a `WARN` at startup for each companion not found (verified from `registry.rs` `discover_all` post-discovery loop). This surfaces incomplete installations without blocking any skill from loading. Users who install only the slim `zencoder` core without the five replacement skills will see five warnings per startup — this is expected and harmless. The warning message per missing companion is:

> Skill 'zencoder' declares companion 'coding' in `requires.skills`, but it is not loaded.

---

## Verification Approach

### Byte Count Verification (Pre-Ship)

For each SKILL.md file, extract the body (content after the closing `---` of frontmatter) and verify:

```bash
# Extract body and check byte count for each file
# Uses printf to strip trailing newline added by command substitution,
# ensuring consistent byte counts across macOS and Linux.
for skill in coding code-review plan-mode commit delegation zencoder; do
  file="skills/${skill}/SKILL.md"
  body=$(awk '/^---/{n++; if(n==2){found=1; next}} found{print}' "$file")
  body_bytes=$(printf '%s' "$body" | wc -c | tr -d ' ')
  echo "${skill}: ${body_bytes} bytes"
done
```

Pass criteria per skill:

| Skill | `max_context_tokens` | Fail if body > |
|---|---|---|
| `coding` | 1,500 | 12,000 bytes |
| `code-review` | 2,500 | 20,000 bytes |
| `plan-mode` | 2,500 | 20,000 bytes |
| `commit` | 1,000 | 8,000 bytes |
| `delegation` | 1,500 | 12,000 bytes |
| `zencoder` | 1,000 | 8,000 bytes |

### YAML Frontmatter Validation

Verify each frontmatter parses as valid YAML with correct field types:

```bash
python3 -c "
import yaml, re, sys
for skill in ['coding','code-review','plan-mode','commit','delegation','zencoder']:
    with open(f'skills/{skill}/SKILL.md') as f:
        content = f.read()
    # Use a line-anchored regex to extract the frontmatter block.
    # A bare string split on '---' would break if any YAML value contains
    # that substring (e.g. a long description field with em-dash separators).
    fm_match = re.search(r'^---\n(.*?)\n---\n', content, re.DOTALL | re.MULTILINE)
    if not fm_match:
        print(f'FAIL: {skill} -> no valid frontmatter delimiters found', file=sys.stderr)
        sys.exit(1)
    fm = fm_match.group(1)
    data = yaml.safe_load(fm)
    name = data.get('name', '')
    assert re.match(r'^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$', name), f'Invalid name: {name}'
    print(f'OK: {skill} -> name={name}')
"
```

### Load Verification (IronClaw Runtime)

After copying files to `~/.ironclaw/skills/`:

```bash
ironclaw skill list
```

Expected output: each of the six skill names appears in the list with `source: user` (not `bundled`). The five replacement skills must appear instead of — not alongside — the bundled versions.

### Activation Smoke Test

```bash
# coding replacement activates on generic coding request
ironclaw chat "fix the null pointer bug in main.go"
# Expected: coding skill loaded (check ironclaw logs), not zencoder monolith

# zencoder core activates on explicit Zencoder request
ironclaw chat "delegate this to zencoder"
# Expected: zencoder skill loaded

# delegation replacement routes to solve_coding_problem
ironclaw chat "help me delegate fixing the auth bug"
# Expected: delegation skill loaded; routing layer offers solve_coding_problem
```

### Regression Guard

With **no** `zencoder-tool` installed (or in `unavailable` state), activate each replacement skill and verify the output matches the behavior of the unmodified bundled skill (routing layer is a no-op; native content drives the response).

---

## Risks and Mitigations

| Risk | Technical Detail | Mitigation |
|---|---|---|
| Token budget rejection at load | Body exceeds `max_context_tokens × 8` bytes → silent skip, no user-visible error | Byte count verification script in CI/pre-ship checklist |
| YAML parse failure | Invalid YAML in frontmatter → silent skip | yaml.safe_load validation script |
| `[PLAN MODE]` keyword YAML parsing | Unquoted `[PLAN MODE]` would be parsed as YAML flow sequence | Must use double-quoted string: `"[PLAN MODE]"` |
| delegation patterns at cap | Original has exactly 5 patterns (MAX_PATTERNS_PER_SKILL) | Override must preserve all 5 exactly; adding one causes 6th to be silently dropped |
| Flat-file collision (zencoder) | Both `skills/zencoder.SKILL.md` (flat) and `skills/zencoder/SKILL.md` (subdirectory) would declare `name: zencoder`; scan order within a directory is OS-dependent | The flat file `skills/zencoder.SKILL.md` is deleted as an explicit implementation step; git history preserves the original content |
| Version string validation | `version` field validated against `^[a-zA-Z0-9._\-+~]{1,32}$`; `+` is allowed, spaces are not | Use `+` separator as in `"1.0.0+zencoder.1"` (14 chars, valid) |
| Upstream skill update | Bundled skill changes while our override is installed | Upstream version comment enables diff-based update; maintainer compares noted version against current bundled content |
