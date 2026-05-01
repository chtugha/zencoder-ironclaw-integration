# Full SDD workflow

## Configuration
- **Artifacts Path**: `.zenflow/tasks/zencoder-native-skill-override-s-9f74`

---

## Agent Instructions

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: 71420d8b-024d-4021-8c90-61322db41e24 -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Focus on **what** the feature should do and **why**, not **how** it should be built. Do not include technical implementation details, technology choices, or code-level decisions — those belong in the Technical Specification.

Save the PRD to `{@artifacts_path}/requirements.md`.

### [x] Step: Technical Specification
<!-- chat-id: babc1e38-3a28-4c01-8aa2-ac5d0a8a70bd -->

Create a technical specification based on the PRD in `{@artifacts_path}/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Do not include implementation steps, phases, or task breakdowns — those belong in the Planning step.

Save to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Verification approach using project lint/test commands

### [x] Step: Planning
<!-- chat-id: 76cf70b2-b4f8-48e1-a561-0531f18323bd -->

Create a detailed implementation plan based on `{@artifacts_path}/spec.md`.

### [x] Step 1: Fetch upstream bundled skill content from nearai/ironclaw
<!-- chat-id: c9330044-64c0-471a-bd79-c454951b87c3 -->

Fetch the raw body content of all five bundled skills from the `nearai/ironclaw` GitHub repository (main branch) before authoring any SKILL.md file. This ensures verbatim/condensed native content is based on actual upstream source, not guesswork.

Skills to fetch (GitHub path: `nearai/ironclaw/skills/<name>/SKILL.md` or flat `nearai/ironclaw/skills/SKILL.md`):
- `coding` v1.0.0 — `nearai/ironclaw` main branch
- `code-review` v2.0.0 — `nearai/ironclaw` main branch
- `plan-mode` v0.1.0 — `nearai/ironclaw` main branch
- `commit` v1.0.0 — `nearai/ironclaw` main branch
- `delegation` v0.1.0 — `nearai/ironclaw` main branch

Additionally, read the local `skills/zencoder.SKILL.md` and record its `version` field (expected: `1.3.0`). The slim core's version string `"1.3.0+slim.1"` in the spec is derived from this base — if the local file's version differs from `1.3.0`, update the slim core version in Step 7 to match (e.g. if local is `1.4.0`, use `"1.4.0+slim.1"`).

Verification: all five upstream files retrieved; actual byte counts of original bodies noted; version fields match spec expectations (`1.0.0`, `2.0.0`, `0.1.0`, `1.0.0`, `0.1.0`); local monolith version confirmed.

### [x] Step 2: Create `skills/coding/SKILL.md`
<!-- chat-id: 4133c329-b4d2-4d85-b4f2-8547da157c27 -->

Implement the `coding` replacement skill per `spec.md` § `skills/coding/SKILL.md`.

**Frontmatter contract** (must match exactly):
- `name: coding`, `version: "1.0.0+zencoder.1"`, `max_context_tokens: 1500`
- 19 keywords (within cap of 20), 3 exclude_keywords, 2 patterns, 2 tags — all as specified
- No `requires:` block (NFR-1: no gating failures)

**Body structure**:
1. `## Zencoder Routing Layer` section with:
   - Self-contained 3-bullet state classifier (healthy/degraded/unavailable definitions)
   - Coding routing rules: check `task_id` from prior `zencoder-tool` response → call `check_solution_status` before local edits; only offer `solve_coding_problem` when user explicitly signals delegation intent; all other cases fall through
2. `---` separator
3. HTML comment: `<!-- upstream: coding v1.0.0 nearai/ironclaw/skills/coding/SKILL.md @main -->`
4. Verbatim upstream `coding` v1.0.0 body

**Token budget**: `max_context_tokens: 1500` → max body = 12,000 bytes. Target ≤ 2,000 bytes total.

**Verification**: extract body bytes; confirm < 12,000. Confirm YAML parses. Confirm no `requires.bins/env/config`.

### [x] Step 3: Create `skills/code-review/SKILL.md`
<!-- chat-id: 8ea6a382-71a9-488a-a6ea-863915075e2f -->

Implement the `code-review` replacement skill per `spec.md` § `skills/code-review/SKILL.md`.

**Frontmatter contract** (must match exactly):
- `name: code-review`, `version: "2.0.0+zencoder.1"`, `max_context_tokens: 2500`
- 3 keywords, 3 patterns, 3 tags — as specified
- `requires.skills: [github]` — preserved from upstream (advisory only, does not block loading)
- No `requires.bins/env/config`

**Body structure**:
1. `## Zencoder Routing Layer` with:
   - Self-contained 3-bullet state classifier
   - Code-review routing rules: if `task_id` is in scope and review subject relates to that task → attach findings via `update_task` (**`update_task` PATCH fully replaces `description`**: must call `get_task` first to read current description, then pass `existing + "\n\n## Code Review Findings\n" + findings` as the new description); if user asks to track PR → suggest `create_automation`; all other cases fall through
2. `---` separator
3. HTML comment: `<!-- upstream: code-review v2.0.0 nearai/ironclaw/skills/code-review/SKILL.md @main -->`
4. Condensed upstream `code-review` v2.0.0 body (target ≤ 5,000 bytes)

**Condensation rules** (per spec.md):
- Retain: six-lens review structure (3a–3f), severity scale, findings table format, GitHub PR API calls (exact code)
- Remove/shorten: Step 1 preamble prose (keep only the essential async pattern note + code block), repetitive rule prose in Step 5 (keep code block + format rule), verbose "if the PR touches more than 20 files" paragraph → single bullet

**Token budget**: `max_context_tokens: 2500` → max body = 20,000 bytes. Target ≤ 6,000 bytes total.

**Verification**: extract body bytes; confirm < 20,000 and < 6,000 actual. Confirm YAML parses. Confirm `requires.skills: [github]` is present (not `requires.bins`).

### [x] Step 4: Create `skills/plan-mode/SKILL.md`
<!-- chat-id: 07c1788a-eb47-4b51-a06b-fa508fd76d03 -->

Implement the `plan-mode` replacement skill per `spec.md` § `skills/plan-mode/SKILL.md`.

**Frontmatter contract** (must match exactly):
- `name: plan-mode`, `version: "0.1.0+zencoder.1"`, `max_context_tokens: 2500`
- 6 keywords (including quoted `"[PLAN MODE]"` — MUST be double-quoted to avoid YAML flow-sequence parsing), 2 patterns, 3 tags
- No `requires:` block

**Body structure**:
1. `## Zencoder Routing Layer` with:
   - Self-contained 3-bullet state classifier
   - Plan-mode routing rules: if `task_id` in scope → use `get_plan`/`update_plan_step`/`add_plan_steps`/`create_plan` (requires `project_id`; call `list_projects` if absent); do NOT create parallel local memory-doc plan; if no `task_id` → fall through to native plan-mode behavior
2. `---` separator
3. HTML comment: `<!-- upstream: plan-mode v0.1.0 nearai/ironclaw/skills/plan-mode/SKILL.md @main -->`
4. Verbatim upstream `plan-mode` v0.1.0 body (original ~2,900 bytes; no condensation needed)

**Token budget**: `max_context_tokens: 2500` → max body = 20,000 bytes. Target ≤ 4,000 bytes total.

**Multi-word keyword scoring note** (per `spec.md` § Multi-Word Keyword Scoring Behavior): keywords such as `plan mode`, `create a plan`, `make a plan`, `execution plan`, and `step by step plan` are multi-word strings. They score at most 5 pts (substring tier: `message.contains(keyword)`) and never 10 pts (exact-word tier: requires a single message token to equal the full keyword string). This is expected and acceptable — these keywords still contribute to the activation score alongside pattern and tag matches. Activation fidelity (FR-5) is preserved; the scoring ceiling is lower for multi-word entries but the skill activates correctly on the targeted phrases.

**Verification**: extract body bytes; confirm < 20,000. Confirm `"[PLAN MODE]"` is quoted in YAML (not an array). Confirm YAML parses cleanly.

### [x] Step 5: Create `skills/commit/SKILL.md`
<!-- chat-id: 980d8a54-6538-4d5f-95b7-c48b77880074 -->

Implement the `commit` replacement skill per `spec.md` § `skills/commit/SKILL.md`.

**Frontmatter contract** (must match exactly):
- `name: commit`, `version: "1.0.0+zencoder.1"`, `max_context_tokens: 1000`
- 2 keywords, 2 patterns, 2 tags
- No `requires:` block

**Body structure**:
1. `## Zencoder Routing Layer` with:
   - Self-contained 3-bullet state classifier
   - Commit routing rules: check last-known Zencoder task status from conversation context; if `task_id` in scope AND status is `inprogress` or `inreview` AND state is healthy → do NOT commit (remote agent owns branch), warn user once, suggest `check_solution_status`; if user explicitly confirms → proceed; if task is `done`/`cancelled`/absent → fall through to native commit behavior
2. `---` separator
3. HTML comment: `<!-- upstream: commit v1.0.0 nearai/ironclaw/skills/commit/SKILL.md @main -->`
4. Verbatim upstream `commit` v1.0.0 body (original ~810 bytes)

**Token budget**: `max_context_tokens: 1000` → max body = 8,000 bytes. Target ≤ 1,500 bytes total.

**Verification**: extract body bytes; confirm < 8,000. Confirm YAML parses.

### [x] Step 6: Create `skills/delegation/SKILL.md`
<!-- chat-id: ee82b42a-8029-4760-b451-9e9fc6026af2 -->

Implement the `delegation` replacement skill per `spec.md` § `skills/delegation/SKILL.md`.

**Frontmatter contract** (must match exactly):
- `name: delegation`, `version: "0.1.0+zencoder.1"`, `max_context_tokens: 1500`
- 10 keywords, exactly 5 patterns (AT THE LIMIT — adding a 6th causes silent drop), 3 tags
- No `requires:` block

**Critical**: the original bundled `delegation` skill has exactly 5 patterns (the maximum). The override must preserve all 5 exactly — do not add or remove any pattern. Verify this by cross-checking fetched upstream content.

**Body structure**:
1. `## Zencoder Routing Layer` with:
   - Self-contained 3-bullet state classifier
   - Delegation routing rules: if user wants to delegate a coding problem (mentions code/files/functions/APIs/tests/builds/repos) AND state is healthy → call `solve_coding_problem` (not `routine_create`/`memory_write`); if no `project_id` known → call `list_projects` first; if non-coding delegation (calendar/reminder/research/schedule) → fall through to native delegation behavior
2. `---` separator
3. HTML comment: `<!-- upstream: delegation v0.1.0 nearai/ironclaw/skills/delegation/SKILL.md @main -->`
4. Verbatim upstream `delegation` v0.1.0 body (original ~1,100 bytes)

**Token budget**: `max_context_tokens: 1500` → max body = 12,000 bytes. Target ≤ 2,000 bytes total.

**Verification**: extract body bytes; confirm < 12,000. Count patterns in frontmatter — must be exactly 5. Confirm YAML parses.

### [x] Step 7: Create `skills/zencoder/SKILL.md`
<!-- chat-id: 25d4d3f5-3b8b-42a6-b780-d4fa2a5f8c3c -->

Implement the slim Zencoder core per `spec.md` § `skills/zencoder/SKILL.md`.

**Frontmatter contract** (must match exactly):
- `name: zencoder`, `version: "1.3.0+slim.1"`, `max_context_tokens: 1000`
- Exactly 20 keywords (AT THE CAP), exactly 5 patterns (AT THE CAP), 6 tags
- `requires.skills: [coding, code-review, plan-mode, commit, delegation]` (5 companions; advisory only)
- No `requires.bins/env/config`

**Body content** (in priority order for the ≤ 4,000-byte budget):
1. Tool overview: 17 actions in 6 categories (one line each, matching the monolith's categories)
2. Authentication: PAT URL (`https://auth.zencoder.ai`), helper script command, `ironclaw tool auth zencoder-tool` command, rotation note
3. `solve_coding_problem` decision flow: 3-step numbered list
4. `check_solution_status` decision flow: 3-step numbered list
5. Task management: `list_tasks` (status filter), `update_task` (requires `project_id` + `task_id`; at least one of title/description/status must be non-empty), `get_plan` (requires `project_id` + `task_id`)
6. Automation management: `create_automation` (name + schedule format), `toggle_automation`
7. Input constraints: UUID format, status case rules (task lowercase / step PascalCase), 64 KiB text cap
8. Error handling: 6-row table (401 → re-auth, 402 → quota, 429 → Retry-After, 5xx → backoff, network → probe, unavailable → permanent)
9. Resilience state machine: condensed 3-state transition table (healthy/degraded/unavailable) + lazy probe conditions (per-state rules)
10. Cross-reference: one sentence noting routing logic for coding/review/plan/commit/delegation lives in respective replacement skills

**Content omitted vs. monolith** (to reach ≤ 4,000 bytes from 18 KB):
- Verbose per-skill composition rules (all "vs `coding`", "vs `delegation`" etc. prose) → single cross-reference sentence
- Exhaustive composition-with-native-skills section → removed
- Full fallback table (detailed per-action fallbacks) → compressed to essential 6-row error table
- Reconciliation / `pending_zencoder_sync` prose → omitted (referenced in replacement skill routing layers)

**Token budget**: `max_context_tokens: 1000` → max body = 8,000 bytes. Target ≤ 4,000 bytes.

**Verification**: extract body bytes; confirm < 8,000. Count keywords (must be exactly 20) and patterns (must be exactly 5). Confirm `requires.skills` lists all 5 companions. Confirm YAML parses.

### [x] Step 8: Remove `skills/zencoder.SKILL.md`
<!-- chat-id: 79fea5d4-ae19-4738-9f36-b1dfba7a24a3 -->

Delete the flat-layout monolith file to eliminate the OS-dependent scan-order collision with `skills/zencoder/SKILL.md`.

Both files declare `name: zencoder`. When both exist in the same directory level, IronClaw's `discover_from_dir` iterates filesystem entries in OS-dependent order — whichever is scanned first wins. The flat file must be deleted; git history preserves the original content for reference.

**Action**: `git rm skills/zencoder.SKILL.md` (or equivalent file deletion).

**Verification**: confirm `skills/zencoder.SKILL.md` no longer exists. Confirm `skills/zencoder/SKILL.md` exists. No other files modified.

### [x] Step 9: Verify all files (byte count + YAML validation)
<!-- chat-id: cd48f418-0476-41ba-9044-b1102a92b9dc -->

Run the verification scripts defined in `spec.md` § "Verification Approach" and record results.

**Byte count check** (run from repo root):
```bash
for skill in coding code-review plan-mode commit delegation zencoder; do
  file="skills/${skill}/SKILL.md"
  body=$(awk '/^---/{n++; if(n==2){found=1; next}} found{print}' "$file")
  body_bytes=$(printf '%s' "$body" | wc -c | tr -d ' ')
  echo "${skill}: ${body_bytes} bytes"
done
```

Pass criteria:
| Skill | Fail if body > |
|---|---|
| `coding` | 12,000 bytes |
| `code-review` | 20,000 bytes |
| `plan-mode` | 20,000 bytes |
| `commit` | 8,000 bytes |
| `delegation` | 12,000 bytes |
| `zencoder` | 8,000 bytes |

**YAML validation** (requires `python3` with `pyyaml`):
```bash
python3 -c "
import yaml, re, sys
for skill in ['coding','code-review','plan-mode','commit','delegation','zencoder']:
    with open(f'skills/{skill}/SKILL.md') as f:
        content = f.read()
    # Use a line-anchored regex — a bare split('---') would break if any YAML
    # value (e.g. a long description field) contains '---' as a substring.
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

**Structural checks** (manual, per file):
- `coding`: exactly 19 keywords, 2 patterns, 2 tags; no `requires:` block
- `code-review`: exactly 3 keywords, 3 patterns, 3 tags; `requires.skills: [github]` present; no `requires.bins/env/config`
- `plan-mode`: `"[PLAN MODE]"` is a quoted string not a YAML array; 6 keywords, 2 patterns, 3 tags
- `commit`: 2 keywords, 2 patterns, 2 tags; no `requires:` block
- `delegation`: exactly 5 patterns (at cap); 10 keywords, 3 tags; no `requires:` block
- `zencoder`: exactly 20 keywords (at cap), exactly 5 patterns (at cap); `requires.skills` lists all 5 companions

Record actual byte counts and pass/fail per skill in this plan file after running.

**Results**:
- `coding`: 3,250 bytes (limit 12,000) — PASS
- `code-review`: 5,940 bytes (limit 20,000) — PASS
- `plan-mode`: 5,152 bytes (limit 20,000) — PASS
- `commit`: 2,266 bytes (limit 8,000) — PASS
- `delegation`: 3,165 bytes (limit 12,000) — PASS
- `zencoder`: 4,086 bytes (limit 8,000) — PASS
- YAML validation: PASS (all 6 skills)
