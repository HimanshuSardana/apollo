---
title: Skills
date: 2026-09-15
tags: [guide, skills]
---

Skills are reusable workflows — Markdown files that teach the agent a procedure (review checklists, commit conventions, debugging playbooks). Apollo loads them on demand with `/skill:<name>`.

## Using a skill

```text
You: /skill:paseo-committee review this diff for root causes
✓ Loaded skill: paseo-committee
Thinking...
Apollo: ...
```

Apollo reads `~/.agents/skills/<name>/SKILL.md` and prepends it to your message, so the model follows the skill's instructions for the rest of that turn. Anything after the skill name is your actual prompt.

## Tab completion

Skill names complete with Tab, just like filenames:

```text
You: /skill:pas<Tab>
You: /skill:paseo-committee
```

## Writing your own

A skill is a directory with a `SKILL.md` file:

```sh
~/.agents/skills/my-review/
  SKILL.md    # name, when to use it, the procedure
```

Keep it focused: one workflow per skill, concrete steps, no fluff. The agent follows it literally, so write instructions, not essays.
