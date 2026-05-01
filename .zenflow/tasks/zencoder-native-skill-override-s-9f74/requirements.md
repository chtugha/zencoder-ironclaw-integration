# Product Requirements Document
## Zencoder Native Skill Override System

**Version**: 1.0  
**Project**: zencoder-ironclaw-integration  
**Status**: Draft

---

## Background

The current integration provides a single `zencoder.SKILL.md` (the "monolith") that activates only when the user explicitly mentions Zencoder or Zenflow keywords. Generic user intents such as _"fix the bug"_, _"review the changes"_, or _"make a plan"_ trigger IronClaw's native bundled skills — `coding`, `code-review`, `plan-mode`, `commit`, and `delegation` — which have no awareness of Zencoder. As a result, autonomous agent routing to Zencoder is never considered for the most common developer actions unless the user adds an explicit Zencoder prefix.

### Verified IronClaw Skill Override Mechanism

From `crates/ironclaw_skills/src/registry.rs` (confirmed in source, unit-tested in `test_bundled_skill_overridden_by_user`):

Discovery order — **earlier source wins on name collision**:
1. Workspace skills (`<workspace>/skills/`) — `SkillTrust::Trusted`
2. User skills (`~/.ironclaw/skills/`) — `SkillTrust::Trusted`
3. Installed skills (`~/.ironclaw/installed_skills/`) — `SkillTrust::Installed`
4. Bundled skills (compiled into binary) — `SkillTrust::Trusted`

When the registry processes bundled skills, it checks the `seen` set: if a skill name was already loaded from a higher-priority source, the bundled version is silently skipped. Placing a `SKILL.md` in `~/.ironclaw/skills/<name>/SKILL.md` with the same `name:` field as a bundled skill **fully replaces** that bundled skill.

### System Constraints (Verified from Source)

| Constraint | Value | Source |
|---|---|---|
| Skill name pattern | `^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$` | `validation.rs` |
| Max keywords per skill | 20 (silently truncated) | `types.rs` |
| Max patterns per skill | 5 (silently truncated) | `types.rs` |
| Max tags per skill | 10 (silently truncated) | `types.rs` |
| Min keyword/tag length | 3 chars (short tokens filtered) | `types.rs` |
| Max file size | 64 KiB | `types.rs` |
| Token budget check | `prompt_bytes * 0.25 > max_context_tokens * 2` → rejected | `registry.rs` |
| Symlinks | Rejected with warning | `registry.rs` |
| File layout | Both flat (`<skills-dir>/SKILL.md`) and subdirectory (`<skills-dir>/<name>/SKILL.md`) are supported. Subdirectories without `SKILL.md` are recursed as bundle directories (up to depth 3). | `registry.rs` |

**Token budget implication**: For `max_context_tokens: 1500`, the prompt body must be under `1500 × 2 × 4 = 12,000 bytes` (~3,000 tokens). Skills exceeding this fail silently at load time and are skipped — they do not cause an error visible to the user.

---

## Problem Statement

### Root Problem

IronClaw's activation model is per-message: the skill selector scores each skill against the user's message and loads the highest-scoring skills up to the token budget. The `zencoder` monolith only scores high when the user explicitly says "zencoder" or "zenflow". For the most common intents — code edits, reviews, commits, delegation — the native bundled skills win and Zencoder-aware routing is never applied.

### Consequences

1. **No autonomous routing**: When a Zencoder task is already in scope (e.g. a `task_id` exists in context), a plain _"fix the bug"_ message still tries to edit files locally rather than calling `check_solution_status` or `solve_coding_problem`.

2. **High peak token cost**: When the user does invoke Zencoder explicitly, the 3,500-token monolith loads alongside the native coding skill, consuming a disproportionate share of the token budget on every turn.

3. **Incomplete fallback guidance**: The native skills have no fallback instructions for when the user is in the middle of a Zencoder-managed task.

---

## Goals

1. **Every coding turn is Zencoder-aware** — without requiring the user to say "zencoder". When a `task_id` is in context, the appropriate Zencoder action is offered automatically.

2. **Reduce peak token cost** — the combined per-turn skill budget should be equal to or lower than the current monolith (3,500 tokens) on every turn type.

3. **Preserve full native functionality** — users who have not set up Zencoder, or who are offline, must experience identical behavior to the unmodified bundled skills.

4. **Zero regression on IronClaw upgrade** — the override files must remain safe to install even if IronClaw removes or renames the corresponding bundled skills.

---

## Scope

### In Scope

Six SKILL.md files to be created in the repository under `skills/`:

| File | Replaces Bundled | Declared `max_context_tokens` | Target Content Budget |
|---|---|---|---|
| `skills/coding/SKILL.md` | `coding` (v1.0.0) | 1,500 (same as bundled) | ≤ 1,500 tokens |
| `skills/code-review/SKILL.md` | `code-review` (v2.0.0) | 2,500 (same as bundled) | ≤ 1,500 tokens |
| `skills/plan-mode/SKILL.md` | `plan-mode` (v0.1.0) | 2,500 (same as bundled) | ≤ 1,500 tokens |
| `skills/commit/SKILL.md` | `commit` (v1.0.0) | 1,000 (same as bundled) | ≤ 1,000 tokens |
| `skills/delegation/SKILL.md` | `delegation` (v0.1.0) | 1,500 (same as bundled) | ≤ 1,500 tokens |
| `skills/zencoder/SKILL.md` | _(new, no override)_ | 1,000 | ≤ 1,000 tokens |

The `skills/zencoder/SKILL.md` replaces the existing monolith `skills/zencoder.SKILL.md` as the canonical Zencoder-core skill. The existing flat-layout file **must be deleted** as part of the implementation — both files declare `name: zencoder`, and IronClaw's `discover_from_dir` scan order within a directory is OS-dependent (filesystem iteration order). Allowing both to coexist creates undefined behavior for workspace-mode users. Git history preserves the original content for reference.

### Out of Scope

The following bundled skills are **not** overridden. They have no conflict with Zencoder workflows or provide essential downstream side-effects (Health Score, signals, pacing logic) that must remain untouched:

- All `*-setup` skills (one-time, self-excluding via `setup_marker`)
- `commitment-triage`, `commitment-digest`, `decision-capture`, `delegation-tracker`
- `github`, `github-workflow`
- `security-review`, `qa-review`, `review-readiness`, `review-checklist`
- `routine-advisor`, `tech-debt-tracker`, `new-project`, `product-prioritization`
- All persona/vertical skills (`portfolio`, `trader`, `linear`, `llm-council`, etc.)

---

## Functional Requirements

### FR-1: Override File Compatibility

Each replacement SKILL.md must:
- Use the exact same `name:` value as the bundled skill it replaces (e.g. `name: coding`).
- Be placed at `skills/<name>/SKILL.md` in the repository (subdirectory layout, matching IronClaw's expected path).
- Pass all IronClaw parser validation: valid name pattern, non-empty prompt, token budget within `max_context_tokens × 2 × 4 bytes`.
- Not use symlinks (rejected by the registry with a warning).
- Be installable by copying `skills/<name>/SKILL.md` → `~/.ironclaw/skills/<name>/SKILL.md`.

### FR-2: Zencoder Routing Layer (all replacement skills)

Each replacement skill's prompt body must include a **Zencoder Routing Layer** section that comes before the original native content. This section:

- Checks whether a `task_id` (from a current Zencoder-delegated task) is present in the conversation context.
- If `task_id` is present AND `zencoder_state` is `healthy`: routes to the appropriate `zencoder-tool` action instead of local primitives.
- If `task_id` is absent OR `zencoder_state` is degraded: falls through to the original native behavior unchanged.
- Must not interfere with skill activation scoring (routing logic goes in the prompt body, not in keywords/patterns).

**Context detection mechanism**: Neither `task_id` nor `zencoder_state` is injected by IronClaw or `zencoder-tool` via any capability or context variable mechanism. Both values are **model-tracked conversation state** — they appear as fields in prior `zencoder-tool` call results (e.g. `solve_coding_problem` returns a `task_id`; every tool call result reveals HTTP status for state classification) and the model is instructed to carry them forward in its internal reasoning across turns. The routing layer prompt in each replacement skill must therefore phrase its conditions as "if a `task_id` from a prior `zencoder-tool` response is present in the conversation" rather than referencing any programmatic variable injection.

**Self-contained state classification**: Because the `zencoder` core skill (which defines the full resilience state machine) may not be loaded on the same turn as a replacement skill, each replacement skill's routing layer must include a **minimal, self-contained state classification rule** — not a cross-reference. This ensures the routing layer can evaluate `zencoder_state` even when the `zencoder` core is absent from the prompt. The minimal rule is:

- `healthy`: the last `zencoder-tool` call in this conversation returned HTTP 2xx, or no `zencoder-tool` call has been made yet (default).
- `degraded`: the last `zencoder-tool` call returned HTTP 401, 402, 429, 5xx, or a network/timeout error.
- `unavailable`: `zencoder-tool` is not installed (tool-not-found error).

When `degraded` or `unavailable`, skip the Zencoder routing and fall through to native behavior. The full state machine (retry logic, exponential backoff, recovery probing) remains in the slim `zencoder` core skill and is not duplicated.

### FR-3: Native Content Preservation

Each replacement skill's prompt body must include the original native skill content (clearly delimited from the Zencoder additions), so:

- Users without `zencoder-tool` installed receive equivalent guidance to the unmodified bundled skill.
- When IronClaw ships improvements to a bundled skill, the original section can be updated by a text swap.
- The upstream version is noted in a comment for change-tracking.

When the target content budget is lower than the original skill's content (e.g. `code-review` and `plan-mode` target ≤ 1,500 tokens but the originals are ~2,500 tokens), the native content must be **condensed** — preserving all behavioral rules and decision logic while trimming verbose examples, repeated explanations, and edge-case prose. The condensed version must produce functionally identical model behavior for the same inputs.

### FR-4: Token Budget Compliance

Each skill must fit within its declared `max_context_tokens` budget constraint (prompt body < `max_context_tokens × 2 × 4` bytes). Priority order when space is tight:

1. Zencoder routing layer (Zencoder-aware additions — concise, mandatory)
2. Core native workflow guidance (condensed from original where needed)
3. Edge-case rules and examples (optional, trim if over budget)

**Frontmatter `max_context_tokens` rule**: Each replacement skill must declare the **same `max_context_tokens` value** as the bundled skill it replaces. This preserves the skill selector's scoring and budget-allocation behavior. Changing this value would alter when and whether the skill is selected relative to competing skills. The actual prompt content may be shorter than the declared budget — that is harmless and expected (the budget is a ceiling, not a target). Specifically:

| Replacement skill | Declared `max_context_tokens` (same as bundled) |
|---|---|
| `coding` | 1,500 |
| `code-review` | 2,500 |
| `plan-mode` | 2,500 |
| `commit` | 1,000 |
| `delegation` | 1,500 |
| `zencoder` (slim core) | 1,000 (new; no bundled counterpart) |

### FR-5: Activation Fidelity

Each replacement skill must activate on the same messages as the bundled skill it replaces. This requires:

- Preserving the original `keywords`, `exclude_keywords`, `patterns`, and `tags` from the upstream frontmatter (within the 20/5/10/3 limits enforced by IronClaw).
- Not adding Zencoder-specific keywords (e.g., "zencoder", "zenflow") to replacement skills — those remain on the standalone `zencoder` skill to prevent double-loading.

### FR-6: Slim Zencoder Core (`skills/zencoder/SKILL.md`)

The new slim core skill replaces the monolith (`skills/zencoder.SKILL.md`) as the primary Zencoder skill. It must:

- Retain the `name: zencoder` identifier and all activation keywords/patterns from the monolith.
- Contain: tool overview (17 actions), authentication instructions, key decision flows (`solve_coding_problem`, `check_solution_status`, task management), input constraints, error handling, and the resilience state machine.
- Omit: verbose composition rules for every native skill (those are now embedded in the respective replacement skills). A brief cross-reference is sufficient.
- Target `max_context_tokens: 1000` (down from 3,500 in the monolith).

### FR-7: Companion Skill Declaration

The slim `zencoder` skill must declare the five replacement skills as companions using IronClaw's `requires.skills` frontmatter field. This causes IronClaw to emit a warning log for each companion not found at startup, surfacing incomplete installations without blocking load. (Per IronClaw design: `requires.skills` is advisory only — the skill loads regardless of whether companions are present. Capped at `MAX_REQUIRED_SKILLS_PER_MANIFEST` = 10 entries per `types.rs`.)

### FR-8: Upgrade Safety

Each replacement file must include the upstream SKILL.md version it was derived from (in a comment in the frontmatter or body). When the upstream bundled skill changes, the maintainer can compare versions and merge improvements into the original-content section without rewriting the Zencoder additions.

### FR-9: Fallback Behavior

When `zencoder_state` is anything other than `healthy`, replacement skills must:

- Fall through to the original native behavior (not block or error).
- Not attempt to call `zencoder-tool` actions.
- Include the minimal state classification rule from FR-2 (3 bullet points: `healthy`/`degraded`/`unavailable`) so the routing layer is self-contained and does not depend on the `zencoder` core skill being loaded on the same turn.
- Not duplicate the full resilience state machine (retry logic, exponential backoff, recovery probing, reconciliation) — those remain exclusively in the slim `zencoder` core skill.

---

## Per-Skill Behavior Requirements

### Coding Override (`skills/coding/SKILL.md`)

**Zencoder routing layer behavior**:
- If a `task_id` is in scope and `zencoder_state` is `healthy`: call `check_solution_status` before reaching for local edit primitives. Only edit locally when the user explicitly says "do it yourself" or when `check_solution_status` confirms there is no active remote task for this work.
- If no `task_id` is in scope and the user asks to fix/implement something: offer `solve_coding_problem` as the primary path if `zencoder-tool` is available.

**Native fallback**: All original `coding` skill content (tool usage discipline, code change discipline, code quality rules).

### Code Review Override (`skills/code-review/SKILL.md`)

**Zencoder routing layer behavior**:
- If the review subject is a branch owned by an active Zencoder task: attach findings via `update_task` (append to description) rather than a standalone review document.
- If the review is for a GitHub PR from a Zencoder-tracked branch: prefer creating a `create_task_automation` for ongoing tracking over a one-off review.
- For all other cases: fall through to original review behavior.

**Native fallback**: All original `code-review` skill content (six-lens review, GitHub PR path, local path, posting comments).

**Token budget note**: The original `code-review` skill declares `max_context_tokens: 2500` and contains substantial content. The replacement declares the same `max_context_tokens: 2500` in frontmatter (per FR-4, to preserve selector scoring) but targets a content budget of ≤ 1,500 tokens. This requires condensing the original content — detailed code examples in the posting section and verbose edge-case descriptions should be trimmed while preserving all behavioral rules and the six-lens review structure.

### Plan Mode Override (`skills/plan-mode/SKILL.md`)

**Zencoder routing layer behavior**:
- If a `task_id` is in scope: use `get_plan`, `update_plan_step`, and `add_plan_steps` as the canonical plan store. Do not create a parallel local memory-doc plan.
- If no `task_id` is in scope: fall through to original `plan-mode` behavior (memory-doc plans, `plan_update`, missions).

**Native fallback**: All original `plan-mode` skill content (creating, approving, executing, revising, and listing plans).

### Commit Override (`skills/commit/SKILL.md`)

**Zencoder routing layer behavior**:
- If a Zencoder task is `inprogress` or `inreview`: do not commit locally. The remote agent owns the branch. Warn the user and suggest checking `check_solution_status` first.
- If the task is `done`, `cancelled`, or no task is in scope: fall through to original commit behavior.

**Native fallback**: All original `commit` skill content (staging review, message generation, secret detection, user confirmation).

### Delegation Override (`skills/delegation/SKILL.md`)

**Zencoder routing layer behavior**:
- If the user wants to delegate a **coding problem** (bug fix, feature, refactor, test): call `solve_coding_problem` as the primary path, not `routine_create` or `memory_write`.
- If the user wants to delegate a **non-coding task** (calendar, reminder, research, schedule): fall through to original delegation behavior.
- Distinction heuristic: any delegation request mentioning code, files, functions, APIs, tests, builds, or repositories is treated as a coding problem.

**Native fallback**: All original `delegation` skill content (clarify, break down, track, report back).

### Slim Zencoder Core (`skills/zencoder/SKILL.md`)

**Content requirements** (in priority order for token budget):
1. Tool overview (17 actions, categorized)
2. Authentication instructions (PAT generation, `ironclaw tool auth`, token rotation)
3. `solve_coding_problem` decision flow
4. `check_solution_status` decision flow
5. Task management operations (`list_tasks`, `update_task`, `get_plan`)
6. Automation management (`create_automation`, `toggle_automation`)
7. Input constraints (UUID format, status values, text caps)
8. Error handling table (401, 402, 429, 5xx, network, unavailable)
9. Resilience state machine (condensed — state transitions and fallback table only)
10. Cross-reference to replacement skills (single sentence)

**Content to omit from the monolith**:
- Verbose per-skill composition rules (now embedded in replacement skills)
- Exhaustive fallback descriptions (condensed to a table)
- Repetitive state-machine prose (replaced by a compact table)

---

## Non-Functional Requirements

### NFR-1: No External Dependencies

Replacement skills must not declare `requires.bins`, `requires.env`, or `requires.config` constraints. They must load on any IronClaw installation where the corresponding bundled skill loaded.

### NFR-2: No Credential Declarations

Replacement skills must not declare credential specifications in their frontmatter. The `zencoder_access_token` credential is managed by the `zencoder-tool` capabilities manifest and injected by IronClaw's credential system. Duplicating it in a skill frontmatter would register redundant host-to-credential mappings.

### NFR-3: Idempotent Installation

Copying any combination of the six skill files to `~/.ironclaw/skills/` must produce correct behavior. Users who install only some replacements get a partially-aware setup (those installed are Zencoder-aware; uninstalled ones fall back to bundled behavior). There must be no crash or parse error from partial installation.

### NFR-4: Qwen 3 9B Compatibility

Prompt content must be written for smaller models (7–14B parameter range). This means:
- Explicit routing rules rather than implied judgment calls.
- Numbered or bulleted lists over prose paragraphs.
- Concrete action names (`solve_coding_problem`, `check_solution_status`) cited rather than described generically.
- Condition-action pairs for routing decisions (e.g. "if X → do Y; else → do Z").

### NFR-5: No Gating Failures

Replacement skills must not fail gating. They must not declare requirements that could be absent on user machines. The only acceptable `requires:` block is `requires.skills` (advisory companion declarations on the `zencoder` core, which do not block loading).

---

## Assumptions

1. **Workspace directory**: This repository's `skills/` directory maps to the IronClaw workspace skills path (`<workspace>/skills/`). Skills placed here take highest priority over user and bundled skills. The implementation targets the user installation path (`~/.ironclaw/skills/`) for end users, and the workspace path for contributors running directly from this repo.

2. **Monolith deletion required**: The existing `skills/zencoder.SKILL.md` (the monolith, flat-layout) **must be deleted** as part of the implementation. IronClaw supports both flat (`SKILL.md` directly in the skills directory) and subdirectory (`<name>/SKILL.md`) layouts within the same directory — confirmed by `discover_from_dir` in `registry.rs` and the `test_load_flat_layout` / `test_mixed_flat_and_subdirectory_layout` unit tests. However, when a flat file and a subdirectory file both declare the same `name: zencoder` in the same parent directory, IronClaw's scan order is determined by the underlying filesystem's directory entry iteration order — this is not guaranteed and is OS-dependent. Whichever file is scanned first wins, creating undefined behavior that cannot be controlled by the repository author. Deleting the flat file (`skills/zencoder.SKILL.md`) so that only `skills/zencoder/SKILL.md` exists is the only safe implementation path. Git history preserves the original monolith content for reference. Users who previously installed the monolith as a flat file at `~/.ironclaw/skills/zencoder.SKILL.md` must also remove it when upgrading to the split layout.

3. **`zencoder_access_token` is available**: Replacement skills assume `zencoder-tool` is installed and `zencoder_access_token` is configured when the Zencoder routing layer fires. Each replacement skill handles the `unavailable` case (tool not installed) via the minimal state classification rule required by FR-2 — when `unavailable`, the routing layer is skipped and native behavior runs. The full resilience state machine (retry, backoff, recovery probing) remains exclusively in the `zencoder` core.

4. **IronClaw version**: IronClaw 0.25 or later is required. The override mechanism, subdirectory layout, and `requires.skills` advisory field are all confirmed present in the `main` branch (0.27.0 as of the writing of this PRD).

---

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Upstream bundled skill updates | Our override freezes the original content at the version it was derived from | Note the upstream version in each file; provide a documented update procedure |
| Token budget rejection at load time | Skill silently skipped; no error shown to user | Verify byte count for each file before shipping; add an install-time check script |
| Partial installation | Some turns are Zencoder-aware, others are not | `requires.skills` companion warning surfaces the gap in logs |
| Bundled skill deletion | Override becomes a standalone skill with the same name | No crash; behavior degrades gracefully to Zencoder-only guidance |
| Name collision in workspace dir | If the repo is used directly as the workspace, workspace-level skills override user skills | Expected and desirable — workspace always wins |

---

## Success Criteria

1. All six SKILL.md files parse and load without error on IronClaw 0.25+.
2. With all six replacement files installed, a plain _"fix the bug"_ message activates the `coding` replacement skill, not the bundled `coding` skill.
3. When a `task_id` from a prior `zencoder-tool` response is present in conversation context and `zencoder_state` is `healthy`, the replacement `coding` skill's routing layer causes the model to call `check_solution_status` rather than reaching for local file-edit tools (`apply_patch` / `read_file` / `write_file`).
4. When no `task_id` is in context, the replacement skills produce identical model behavior to the unmodified bundled skills (the Zencoder routing layer is a no-op).
5. The slim `zencoder` core loads at ≤ 1,000 declared tokens.
6. Each replacement skill loads within its declared `max_context_tokens` budget (verified by byte count).
7. A user with no `zencoder-tool` installed experiences identical behavior to unmodified IronClaw (fallback path).
