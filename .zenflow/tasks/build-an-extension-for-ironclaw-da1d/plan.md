# Full SDD workflow

## Configuration
- **Artifacts Path**: `.zenflow/tasks/build-an-extension-for-ironclaw-da1d`

---

## Agent Instructions

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: b4edebdd-65e0-4cb9-bf21-bec15f26550c -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Focus on **what** the feature should do and **why**, not **how** it should be built. Do not include technical implementation details, technology choices, or code-level decisions — those belong in the Technical Specification.

Save the PRD to `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/requirements.md`.

### [ ] Step: Technical Specification

Create a technical specification based on the PRD in `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Do not include implementation steps, phases, or task breakdowns — those belong in the Planning step.

Save to `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Verification approach using project lint/test commands

### [ ] Step: Planning

Create a detailed implementation plan based on `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/spec.md`.

1. Break down the work into concrete tasks
2. Each task should reference relevant contracts and include verification steps
3. Replace the Implementation step below with the planned tasks

Rule of thumb for step size: each step should represent a coherent unit of work (e.g., implement a component, add an API endpoint). Avoid steps that are too granular (single function) or too broad (entire feature).

Important: unit tests must be part of each implementation task, not separate tasks. Each task should implement the code and its tests together, if relevant.

If the feature is trivial and doesn't warrant full specification, update this workflow to remove unnecessary steps and explain the reasoning to the user.

Save to `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/plan.md`.

### [ ] Step: Implementation

This step should be replaced with detailed implementation tasks from the Planning step.

If Planning didn't replace this step, execute the tasks in `.zenflow/tasks/build-an-extension-for-ironclaw-da1d/plan.md`, updating checkboxes as you go. Run planned tests/lint and record results in plan.md.
