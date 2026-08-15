# GitHub Project Playbook

The [Technical TTS project](https://github.com/users/rsstdd/projects/1) is the system of work. ADRs and repository documents remain the system of record for decisions.

## Item model

- Epics are issues labeled `type:epic`.
- Stories are sub-issues labeled `type:story`, `epic:E*`, and one primary `track:*`.
- Story tasks remain checkboxes in the story because they are implementation steps, not independently scheduled deliverables.
- New issues require an explicit reason; do not duplicate a task checkbox as an issue unless it gains an independent owner, dependency, or acceptance result.

## Status policy

| Status | Entry condition | Exit condition |
|---|---|---|
| Todo | Defined but not actively implemented | Definition of ready confirmed |
| In Progress | One owner is implementing or collecting active evidence | Pull request/evidence is ready or a blocker is external |
| Done | Definition of done is satisfied | Reopen only with a recorded regression or invalid evidence |

Blocked work remains visible with a blocker comment, owner, next action, and review date. Status is not a substitute for the blocker record.

## Story start checklist

1. Confirm prerequisites and definition of ready.
2. Assign the story and move it to In Progress.
3. Link the controlling ADR section and relevant documents.
4. Identify the first failing test or evidence protocol.
5. Record any open question that can change behavior.
6. Create a branch named `story/<story-id>-<short-name>`.

## Story close checklist

1. Check every completed task in the issue body.
2. Link the pull request or commit.
3. Link test output and governed evidence.
4. State human-listening status when speech changed.
5. Record documentation, schema, and compatibility effects.
6. Confirm the walking skeleton remains green.
7. Confirm no unresolved acceptance work is hidden.
8. Move the story to Done; the parent epic progress updates automatically.

## Required filters

Maintain views or filters for:

- current milestone;
- status by epic;
- track by status;
- blocked and awaiting evidence;
- release-gate work;
- all `backlog:v3` items.

Labels route ownership and search. They do not authorize a release or replace acceptance evidence.

