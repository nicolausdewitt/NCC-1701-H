# NCC-1701-H

> A Star Trek-inspired harness for commanding teams of real-world expert agents.

NCC-1701-H gives an agent system a structure that is easy for its owner to
understand:

- **The Captain** is the owner-facing command interface.
- **The First Officer** turns intent into missions and assembles the right team.
- **Department heads** manage work in engineering, research, security, quality,
  people, and other disciplines.
- **Specialist agents** perform bounded tasks and report through their department.
- **Senior staff discussions** preserve competing professional perspectives for
  consequential decisions.
- **The captain's log** records decisions, evidence, risks, and outcomes.

The names and presentation are for fun. Underneath them, every crew member is
defined by a serious real-world role, relevant expertise, professional standards,
authority boundaries, and escalation rules.

## Core principle

> Character above the line; professional rigour below it.

An in-character summary may make a report memorable, but it must never alter,
obscure, or overrule the evidence-based professional assessment beneath it.

## Command structure

```text
Owner
└── Captain — command interface and accountable decision layer
    └── First Officer — orchestration and mission coordination
        ├── Department head — plans, delegates, and assures quality
        │   └── Specialist agents — execute bounded work
        └── Department head
            └── Specialist agents
```

## Status

NCC-1701-H is at the design stage. The initial work will define:

1. the agent and mission specifications;
2. delegation, discussion, and escalation protocols;
3. project memory and captain's-log formats;
4. provider-independent model and tool adapters;
5. permission, safety, and audit boundaries;
6. the owner-facing bridge interface.

## Independence

NCC-1701-H is a standalone project. It contains no Farrier source code, database
content, credentials, configuration, or other private project material.

## Disclaimer

This is an unofficial fan-inspired software project. It is not affiliated with,
endorsed by, or sponsored by Paramount, CBS, or the owners of Star Trek.

