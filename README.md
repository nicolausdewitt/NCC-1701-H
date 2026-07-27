# NCC-1701-H

> A native agent-command application that makes multidisciplinary AI teams
> understandable, configurable, and fun.

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
+-- Captain - command interface and accountable decision layer
    +-- First Officer - orchestration and mission coordination
        +-- Department head - plans, delegates, and assures quality
        |   +-- Specialist agents - execute bounded work
        +-- Department head
            +-- Specialist agents
```

## Status

NCC-1701-H is in early development as a native Rust desktop application. It is
not an Electron or Store application. Tauri compiles the application core into
a native binary and renders the bridge with the operating system webview;
Tokio runs model, tool, process, filesystem, and persistence I/O away from the
render thread.

The owner interacts through an original LCARS-style Three.js bridge. Its visual
language is functional: department colours, command rails, status bands, 3D
spaces, and live activity indicators expose how the agent organisation is
operating. Audit reviews can become briefing-room walkthroughs while retaining
structured evidence beneath every visual element.

The interface may use game-design techniques, but game memory is only a fast,
disposable projection. Every meaningful UI action is committed immediately
through the Rust Warp Core to local SQLite, then relayed to base from a durable
idempotent outbox. Engineering agents can perform isolated work on GitHub
branches or worktrees without confusing source-control state with operational
application data.

The Cargo workspace separates:

- `ncc-core`: provider-independent crew, mission, and model contracts;
- `ncc-orchestrator`: bounded asynchronous command and event flows;
- `ncc-bridge`: the Tauri shell and Three.js desktop interface.

Each team leader has an independent provider and model assignment. This allows,
for example, Engineering and Research to use different models without coupling
the professional role to a particular vendor.

The initial work will define:

1. the agent and mission specifications;
2. delegation, discussion, and escalation protocols;
3. project memory and captain's-log formats;
4. provider-independent model and tool adapters;
5. permission, safety, and audit boundaries;
6. the owner-facing bridge interface.

## Development

Install the stable Rust toolchain and pnpm, then:

```console
cargo test --workspace
cd apps/ncc-bridge
pnpm install
pnpm tauri dev
```

For an optimised native installer:

```console
pnpm tauri build
```

## Independence

NCC-1701-H is a standalone project. It contains no Farrier source code, database
content, credentials, configuration, or other private project material.

## Disclaimer

This is an unofficial fan-inspired software project. It is not affiliated with,
endorsed by, or sponsored by Paramount, CBS, or the owners of Star Trek.
