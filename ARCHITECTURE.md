# Architecture

## System boundary

NCC-1701-H is a provider-independent agent harness. Character identities form
the human-facing vocabulary; real-world professional specifications govern
agent behaviour.

## Runtime structure

The application is a native Rust Cargo workspace:

- `ncc-core` owns stable domain contracts and has no UI or provider SDK;
- `ncc-orchestrator` owns asynchronous coordination and bounded queues;
- `ncc-bridge` is a Tauri shell with a 2D LCARS meeting interface;
- future provider, tool, and persistence adapters sit behind core traits.

The UI thread never performs model, network, tool, filesystem, process, or
database I/O. It dispatches small commands and consumes immutable events.
Bounded Tokio channels provide explicit backpressure between producers and
consumers.

The bridge uses an original LCARS-style visual system rendered with semantic
HTML and CSS inside the operating system webview. Colour, rails, officer
stations, status blocks, and restrained motion communicate live orchestration
state; they are not merely decorative theming.

Interface state is an expendable in-memory projection. It can always be
reconstructed from Warp Core read models and must never be the only copy of
owner work.

The native Rust process owns all external I/O, secrets, persistence, tools, and
agent execution. Tauri commands carry bounded owner requests. Tauri channels
will carry high-rate streaming output to the interface. A future Rust-to-WASM
protocol module may own deterministic client-side state transforms, but it will
not hold credentials or replace the native I/O boundary.

Audit reviews are structured records first and visual briefings second. The
meeting view may navigate findings, illuminate the responsible department, and
display debate, but every statement must remain traceable to its source evidence
and remediation owner.

## Model assignment

Model choice is configuration, not character identity. Every team leader has an
independent assignment consisting of:

- provider adapter identifier;
- provider-specific model identifier;
- optional endpoint override.

Changing one leader's assignment does not alter another leader. Credentials are
never part of this assignment and will be resolved through a separate secret
store.

Commissioning is a gated, full-content-panel walkthrough before the operational
LCARS navigation appears. The owner signs in to GitHub, chooses a read-only
project connection, authenticates OpenAI, briefs the command model, and reviews
the proposed crew before commissioning the ship. Account authentication,
repository selection, and repository write authority are deliberately separate
decisions.

The initial OpenAI adapter reuses Codex's supported **Sign in with ChatGPT**
browser flow and shared credential cache. NCC stores only
`codex-cli:shared-login`; ChatGPT credentials and access tokens never enter the
LCARS webview or Warp Core.

The command model may inspect the generic project context and recommend
provider, model, tool, and focus assignments for each department. Those
recommendations remain proposals until the owner approves them; a command model
cannot silently grant credentials or expand agent authority.

## Project adapter boundary

The harness never imports an application's source code or database schema.
Instead, the Warp Core stores a small, generic `ProjectConnection` containing:

- a project adapter identifier;
- display name and repository locator;
- optional local checkout path;
- default branch.

The connection contains no token. Git and provider credentials are resolved by
native adapters outside the webview. A public `github` or `local-git` adapter
can serve ordinary repositories, while a private project may supply an external
adapter without modifying or forking the harness.

Every newly connected project is read-only. GitHub account sign-in permits
identity verification and repository discovery only. Write access requires a
separate, explicit owner authorization and an opaque reference to credentials
held by the operating-system secret store. A repository URL, successful read,
or staffing brief can never imply permission to create branches, commits,
issues, pull requests, or releases. GitHub authorization uses a native
OAuth/GitHub App or credential-manager flow; passwords and tokens are never
collected by the LCARS webview or written into Warp Core.

The first native credential provider delegates browser authorization and token
custody to GitHub CLI. The Rust process uses the CLI-backed GitHub API to read
the signed-in account and repository list. When write authority is requested,
it also checks the exact repository permission. Warp Core upgrades the
connection to `read_write` only for `WRITE`, `MAINTAIN`, or `ADMIN`, and stores
only an opaque `gh-cli:github.com/<account>` profile reference.

Project adapters translate between this stable contract and a repository's
actual issue tracker, commands, tests, documents, and deployment systems. The
crew and UI depend only on the contract. A private codebase is therefore a
project connected at runtime, not a dependency, built-in mode, or special case
in NCC-1701-H.

## Command flow

1. The owner connects a project through a generic adapter.
2. The owner connects the command model and approves a proposed crew configuration.
3. The owner gives an objective to the Captain.
4. The Captain establishes intent, constraints, and the required authority.
5. The First Officer decomposes the objective into missions.
6. The First Officer activates the relevant department heads.
7. Department heads create bounded specialist assignments.
8. Specialists return evidence and artefacts to their department heads.
9. Department heads review their work and submit independent assessments.
10. For consequential decisions, the First Officer convenes a senior staff review.
11. The Captain presents the recommendation, disagreements, risks, and approvals
   required to the owner.
12. The final decision and outcome enter the captain's log.

## Agent layers

Every named agent has two deliberately separate layers:

### Identity layer

- display name and character inspiration;
- restrained voice and reporting style;
- short, optional in-character summary.

### Professional layer

- real-world role and scope;
- expertise and operating standards;
- required evidence and verification;
- tools and contextual access;
- permitted actions;
- escalation and approval boundaries;
- output and hand-off contracts.

Removing the identity layer must not reduce the professional agent's competence.

## Discussion protocol

Material decisions should not be produced by immediate consensus. Relevant
department heads first assess the problem independently. The discussion then
records:

- agreements;
- conflicting recommendations;
- assumptions and supporting evidence;
- risks and mitigations;
- minority opinions;
- the decision owner.

## Safety boundary

Delegation does not expand authority. A child agent can receive only the
permissions and scope necessary for its assignment, and no agent may bypass an
approval requirement by delegating the action to another agent.

## Endpoint Starship and Warp Core

The Starship is the application runtime installed on an endpoint device. It is
local-first and remains operational without contact with base.

All UI input is intent, not state. The Tauri bridge exposes no direct database
handle and owns no mutable domain store. Every create, edit, delete, setting
change, model assignment, mission action, and audit decision enters the Warp
Core as a typed command. Read models shown by the UI are projections returned
or streamed by the Warp Core.

Its Rust Warp Core owns:

- a SQLite document store containing the ship's current local operating state;
- an append-only ship event log;
- a durable outbox of idempotent server-save commands;
- acknowledgement, rejection, retry, and recovery state;
- replication checkpoints and convergence diagnostics.

A user-visible save succeeds only after the local document, event, and outbound
command commit in one transaction. Network transmission never participates in
that transaction. Base can be absent, slow, or unreachable without invalidating
work completed aboard the ship.

Every server-save command has a stable command ID. Base must deduplicate that ID
before applying the command and return an explicit acknowledgement, permanent
rejection, or merge result. An acknowledgement marks a local document clean only
when no newer local version exists.

The Warp Core also owns the bounded relay loop. A base adapter supplies transport
and authentication only; it cannot reorder commands or change queue state.
Exactly one relay runs per ship. Permanent rejection records the failure and
continues, while transient rejection or transport loss requeues the current
command and stops the drain without touching later commands.

After an unclean shutdown, commands left in `transmitting` return to `queued`.
Permanent rejections remain visible for remediation but do not block later
commands. Transient transport failure stops the current drain and retries later,
so a long outage cannot exhaust every queued command.

SQLite runs in WAL mode with full synchronous durability. Losing or physically
destroying the endpoint can still destroy changes that have never reached base;
the interface must therefore expose replication health and the age of the oldest
unacknowledged command plainly.

## Base and GitHub

Base owns fleet-wide operational persistence, acknowledgement, and recovery.
GitHub owns versioned source code and review artefacts. Engineering missions may
receive isolated branches or worktrees, potentially several in parallel, but a
Git branch is never used as an operational database or replication checkpoint.
