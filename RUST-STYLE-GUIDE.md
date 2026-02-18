# Rust Style Guide

Conventions for Spacebot. Follow these exactly. When in doubt, consistency with existing code wins over personal preference.

## Project Structure

Single binary crate. No workspace, no sub-crates. Library root is `src/lib.rs`, binary entry is `src/main.rs`.

```
src/
├── main.rs             — CLI entry, config loading, startup
├── lib.rs              — re-exports, shared types
├── config.rs
├── error.rs
├── agent/
│   ├── channel.rs
│   ├── branch.rs
│   ├── worker.rs
│   ├── compactor.rs
│   ├── cortex.rs
│   └── status.rs
├── tools/
│   ├── reply.rs
│   ├── branch.rs
│   └── ...
├── memory/
│   ├── store.rs
│   ├── types.rs
│   └── ...
└── ...
```

Never create `mod.rs` files. Use `src/memory.rs` as the module root, not `src/memory/mod.rs`. The module root file contains `mod` declarations and re-exports:

```rust
mod store;
mod types;
mod search;
mod lance;
mod embedding;
mod maintenance;

pub use store::*;
pub use types::*;
pub use search::*;
```

Prefer implementing functionality in existing files unless it's a new logical component. Don't create many small files.

## Lint Configuration

Enforce these clippy lints in `Cargo.toml`:

```toml
[lints.clippy]
dbg_macro = "forbid"
todo = "forbid"
unimplemented = "forbid"
```

`dbg!` and `todo!`/`unimplemented!` should never ship. Use `tracing::debug!` for debug output. Use `// TODO:` comments for tracked future work instead of `todo!()` panics.

## Imports

Grouped into tiers separated by blank lines. Alphabetical within each tier.

```rust
// 1. Crate-local imports
use crate::agent::ProcessType;
use crate::memory::{Memory, MemoryStore, MemoryType};

// 2. External crates (alphabetical by crate name)
use anyhow::{Context as _, Result, anyhow};
use futures::{FutureExt as _, StreamExt as _};
use rig::agent::Agent;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::{mpsc, watch};

// 3. Standard library
use std::sync::Arc;
use std::time::Duration;
```

Key patterns:
- **Suppress unused trait warnings with `as _`:** `use anyhow::Context as _;`, `use futures::FutureExt as _;`
- **Group nested imports** into `{}` blocks from the same crate
- **Alias long crate names** when it improves readability: `use agent_client_protocol as acp;`

## Naming

| Kind | Convention | Examples |
|------|-----------|----------|
| Variables | `snake_case`, full words, no abbreviations | `channel_history`, `worker_status`, `memory_store` |
| Functions (actions) | `snake_case`, verb-first | `spawn_worker`, `save_memory`, `build_status_block` |
| Functions (getters) | `snake_case`, noun-first | `fn model(&self)`, `fn title(&self)` |
| Boolean getters | `is_`/`has_` prefix | `fn is_active(&self)`, `fn has_pending_branches(&self)` |
| Types | `PascalCase`, descriptive, no abbreviations | `ChannelManager`, `MemoryStore`, `WorkerStatus` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_CONCURRENT_BRANCHES`, `COMPACTION_THRESHOLD` |
| Type aliases | `type` keyword for clarity | `pub type ChannelId = Arc<str>;` |

Never abbreviate variable names. `queue` not `q`, `message` not `msg`, `channel` not `ch`. Common abbreviations like `config` are fine when universally understood.

## Struct Definitions

**Derive ordering:** `Debug`, `Clone`, then serialization/comparison traits.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
```

**Field ordering convention:**
1. Identity fields (`id`, `name`)
2. State/data fields
3. Handles to shared resources (`memory_store`, `llm_manager`)
4. Configuration (`model_name`, `max_turns`)
5. Internal state (running tasks, pending operations)
6. Channel senders/receivers (always last)

```rust
pub struct Channel {
    id: ChannelId,
    title: Option<String>,
    history: Vec<rig::message::Message>,
    memory_store: Arc<MemoryStore>,
    llm_manager: Arc<LlmManager>,
    model_name: String,
    max_concurrent_branches: usize,
    active_branches: Vec<JoinHandle<()>>,
    event_tx: mpsc::Sender<ProcessEvent>,
}
```

**`#[non_exhaustive]`** on public structs and enums that may gain fields or variants over time. This allows adding to the type without breaking downstream pattern matches or struct literals:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub usage: TokenUsage,
}
```

Only use this on types that form a public API boundary. Internal types don't need it.

**Struct initialization** uses shorthand when variable names match fields:

```rust
Self {
    id,
    title: None,
    history: Vec::new(),
    memory_store,
    llm_manager,
    model_name,
    max_concurrent_branches: 3,
    active_branches: Vec::new(),
    event_tx,
}
```

## Dependency Bundles

When a struct needs 4+ shared resource handles (`Arc<T>` fields), group them into a dedicated deps struct. This keeps constructors readable and makes it easy to pass the same set of dependencies to child processes.

```rust
pub struct AgentDeps {
    pub memory_store: Arc<MemoryStore>,
    pub llm_manager: Arc<LlmManager>,
    pub tool_server: ToolServerHandle,
    pub event_tx: mpsc::Sender<ProcessEvent>,
}

pub struct Channel {
    id: ChannelId,
    title: Option<String>,
    history: Vec<rig::message::Message>,
    deps: AgentDeps,
    model_name: String,
    max_concurrent_branches: usize,
    active_branches: Vec<JoinHandle<()>>,
}
```

Expose convenience accessors on the owning struct so callers don't chain through the bundle:

```rust
impl Channel {
    fn memory_store(&self) -> &Arc<MemoryStore> { &self.deps.memory_store }
    fn llm_manager(&self) -> &Arc<LlmManager> { &self.deps.llm_manager }
}
```

## Visibility

Fields are private by default. Use `pub(crate)` for internal cross-module access. Only use `pub` for types and methods that form the actual public API.

```rust
pub struct Worker {
    id: WorkerId,                              // private
    pub(crate) status: WorkerStatus,           // other modules in the crate need this
}

#[cfg(test)]
pub fn active_worker_count(&self) -> usize     // test-only accessor
```

## Comments

Comments explain **why**, never **what**. No organizational or summary comments. No section-divider comments. No comments documenting removed code during refactors.

**Module-level doc comments (`//!`)** at the top of every file. One line describing the module's purpose:

```rust
//! Memory graph storage and retrieval.
```

```rust
//! Tiered compaction strategies for channel context management.
```

```rust
// Good: explains non-obvious behavior
// RRF fusion works on ranks rather than raw scores, which handles the
// different scales of vector and keyword results without normalization.
let fused = reciprocal_rank_fusion(vector_results, fts_results, k: 60);

// Bad: restates the code
// Save the memory to the store
memory_store.save(memory).await?;
```

**Doc comments (`///`)** on public APIs and constants that benefit from context:

```rust
/// Channels at this percentage of context capacity trigger background
/// compaction. The compactor runs in a worker without blocking the channel.
pub const COMPACTION_THRESHOLD: f32 = 0.80;
```

**TODO comments** for tracked future work:

```rust
// TODO: Add per-conversation branch throttling. Currently unbounded.
```

Frame comments in a timeless, neutral way. Avoid `CRITICAL:`, `IMPORTANT FIX:`, or alarmist language. Write comments that read well as permanent documentation, not as a changelog entry.

## Error Handling

**Error organization:** Define a top-level `Error` enum in `src/error.rs` that wraps domain-specific error types via `#[from]`. Domain errors live in their respective modules. The crate root re-exports a `Result` type alias:

```rust
// src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Channel(#[from] ChannelError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
```

```rust
// src/lib.rs
pub use error::{Error, Result};
```

Domain modules define their own error enums when callers need to match on specific variants. The top-level `Error` wraps them all so cross-module boundaries don't need to care about which subsystem failed.

Never silently discard errors with `let _ =`. Always handle them.

**Propagate with `?`:**
```rust
let memory = memory_store.load(id).await?;
```

**Add context with `.context()`:**
```rust
let config = load_config(&path)
    .await
    .with_context(|| format!("failed to load config from {}", path.display()))?;
```

**Log non-critical failures with `tracing`:**
```rust
if let Err(error) = memory_store.save(memory).await {
    tracing::warn!(%error, memory_id = %id, "failed to persist memory");
}
```

**`.ok()` only on channel sends** where the receiver may legitimately be dropped:
```rust
event_tx.send(ProcessEvent::StatusUpdate(status)).ok();
```

**Custom error enums with `thiserror`:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("channel {id} not found")]
    NotFound { id: ChannelId },
    #[error("max concurrent branches ({max}) reached")]
    BranchLimitReached { max: usize },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

Use `anyhow::Result` for application-level code where callers don't need to match on specific variants. Use `thiserror` enums when callers need to handle specific failure modes differently.

**Validation errors** follow a consistent `"can't <action>: <reason>"` pattern:
```rust
anyhow::bail!("can't spawn worker: channel is shutting down");
anyhow::bail!("can't save memory: content is empty");
```

## Function Signatures

**Parameter ordering:**
1. `&self` / `&mut self`
2. Primary data parameters
3. Shared resource handles
4. Configuration/options
5. Callback parameters (last)

```rust
async fn spawn_branch(
    &mut self,
    message: &UserMessage,
    memory_store: &MemoryStore,
    max_turns: usize,
) -> Result<BranchHandle>
```

**Unused parameters** use `_` prefix:
```rust
async fn on_tool_result(&self, _tool_name: &str, _call_id: Option<String>, ...) -> HookAction
```

**Generics** use `impl Trait` in argument position, `where` clauses for multi-bound generics:
```rust
pub fn register_tool(&mut self, tool: impl Into<Arc<dyn Tool>>) { ... }

pub async fn search<F>(query: &str, filter: F) -> Result<Vec<Memory>>
where
    F: Fn(&Memory) -> bool + Send,
```

## Async Patterns

Spacebot runs on tokio. All I/O and inter-process communication is async.

**`tokio::spawn` for independent concurrent work:**
```rust
let handle = tokio::spawn(async move {
    if let Err(error) = run_compaction_worker(history, memory_store).await {
        tracing::error!(%error, "compaction worker failed");
    }
});
```

**Clone before moving into async blocks** using variable shadowing:
```rust
let memory_store = memory_store.clone();
let event_tx = event_tx.clone();
tokio::spawn(async move {
    // memory_store and event_tx are the clones here
    let memories = memory_store.search(&query).await?;
    event_tx.send(ProcessEvent::RecallComplete(memories)).ok();
    Ok::<_, anyhow::Error>(())
});
```

**Fire-and-forget with logged errors:**
```rust
tokio::spawn(async move {
    if let Err(error) = db.save_message(message).await {
        tracing::warn!(%error, "failed to persist message");
    }
});
```

**`JoinHandle` storage prevents cancellation:**
```rust
struct Channel {
    active_branches: Vec<JoinHandle<()>>,          // multiple concurrent
    compaction_task: Option<JoinHandle<()>>,        // optional one-shot
    _heartbeat_loop: JoinHandle<()>,               // underscore = held for lifetime
}
```

If a `JoinHandle` is dropped, the task keeps running. Store handles when you need to cancel or await the result. Detached tasks run independently.

**`tokio::select!` for racing concurrent operations:**
```rust
tokio::select! {
    result = work_future => {
        handle_result(result)?;
    }
    _ = cancellation_rx.changed() => {
        tracing::info!("worker cancelled");
        return Ok(());
    }
    _ = tokio::time::sleep(timeout) => {
        anyhow::bail!("worker timed out after {timeout:?}");
    }
}
```

**`watch::channel` for state signaling:**
```rust
let (status_tx, status_rx) = watch::channel(WorkerStatus::Running);

// Sender: update status
status_tx.send_modify(|status| *status = WorkerStatus::WaitingForInput);

// Receiver: wait for changes
while status_rx.changed().await.is_ok() {
    let current = status_rx.borrow().clone();
    // react to status change
}
```

**`mpsc::channel` for event streams:**
```rust
let (event_tx, mut event_rx) = mpsc::channel(64);

// Consumer loop
while let Some(event) = event_rx.recv().await {
    match event {
        ProcessEvent::BranchResult(result) => { ... }
        ProcessEvent::WorkerStatus(status) => { ... }
    }
}
```

**`broadcast::channel` for multi-consumer events:**
```rust
let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);

// Each subsystem subscribes
let mut shutdown_rx = shutdown_tx.subscribe();
tokio::select! {
    _ = run_channel(channel) => {}
    _ = shutdown_rx.recv() => { tracing::info!("shutting down"); }
}
```

## Trait Design

**Async traits use native RPITIT** (return position `impl Trait` in traits), not `#[async_trait]`. This avoids the `Box<dyn Future>` allocation that `#[async_trait]` introduces:

```rust
pub trait WorkerImpl: Send + Sync + 'static {
    fn execute(
        &mut self, task: &str,
    ) -> impl Future<Output = Result<String>> + Send;

    fn status(&self) -> WorkerStatus;
}
```

If a trait needs to be object-safe (used as `dyn Trait`), provide a companion `Dyn` trait with a blanket impl that boxes the future. Only reach for this when you actually need dynamic dispatch — for traits with a small number of known implementors, just use the static trait directly.

```rust
pub trait WorkerImplDyn: Send + Sync + 'static {
    fn execute<'a>(
        &'a mut self, task: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
}

impl<T: WorkerImpl> WorkerImplDyn for T {
    fn execute<'a>(
        &'a mut self, task: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(WorkerImpl::execute(self, task))
    }
}
```

This lets you write strongly-typed implementations against the static trait while dispatching dynamically through the `Dyn` version.

Group inherent methods first, then trait implementations separately:

```rust
impl Worker {
    pub fn new(id: WorkerId, task: String) -> Self { ... }
    pub fn status(&self) -> &WorkerStatus { ... }
}

impl Drop for Worker {
    fn drop(&mut self) { ... }
}
```

**Trait objects behind `Arc` for shared cross-task access:**
```rust
memory_store: Arc<MemoryStore>,
llm_manager: Arc<LlmManager>,
```

**`Box<dyn Trait>` for owned single-use polymorphism:**
```rust
worker_impl: Box<dyn WorkerImpl>,
```

**Trait bounds use `Send + Sync` when data crosses task boundaries:**
```rust
pub trait WorkerImpl: Send + Sync + 'static {
    async fn execute(&mut self, task: &str) -> Result<String>;
    async fn follow_up(&mut self, message: &str) -> Result<String>;
    fn status(&self) -> WorkerStatus;
}
```

**Associated constants on traits:**
```rust
pub trait Tool: Send + Sync + 'static {
    const NAME: &'static str;
    type Input: DeserializeOwned + Serialize + JsonSchema;
    type Output: Serialize;

    async fn call(&self, input: Self::Input) -> Result<Self::Output>;
}
```

## Pattern Matching

**`let-else` for early returns:**
```rust
let Some(worker) = self.workers.get_mut(&worker_id) else {
    return Err(anyhow!("worker {worker_id} not found"));
};
```

**Prefer exhaustive matching** — list all variants explicitly so new variants cause a compile error instead of silently falling through. Use `_ => {}` only when the enum is `#[non_exhaustive]` (external) or when you genuinely don't care about future variants:

```rust
// Preferred: exhaustive, compiler catches new variants
match event {
    ProcessEvent::BranchResult(result) => self.incorporate_branch(result).await?,
    ProcessEvent::WorkerStatus(update) => self.update_status_block(update),
    ProcessEvent::WorkerComplete(result) => self.handle_completion(result).await?,
}

// Acceptable: when matching on a non_exhaustive or foreign enum
match event {
    ProcessEvent::BranchResult(result) => self.incorporate_branch(result).await?,
    ProcessEvent::WorkerStatus(update) => self.update_status_block(update),
    ProcessEvent::WorkerComplete(result) => self.handle_completion(result).await?,
    _ => {}
}
```

**Destructuring in match arms:**
```rust
match error {
    PromptError::MaxTurnsError { chat_history, max_turns, .. } => {
        tracing::warn!(max_turns, "worker hit turn limit");
        save_history(*chat_history).await?;
    }
    PromptError::PromptCancelled { chat_history, reason } => {
        tracing::info!(%reason, "worker cancelled by hook");
    }
    _ => return Err(error.into()),
}
```

## Serde Patterns

**`#[serde(default)]` for backward-compatible deserialization:**
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    pub prompt: String,
    pub interval_secs: u64,
    #[serde(default)]
    pub active_hours: Option<ActiveHours>,
    #[serde(default)]
    pub notify_on_complete: bool,
}
```

**`#[serde(rename_all = "snake_case")]` for enum variants:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Fact,
    Preference,
    Decision,
    Identity,
    Event,
    Observation,
}
```

**`#[serde(tag = "type")]` for internally tagged enums:**
```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProcessEvent {
    BranchResult { branch_id: Uuid, conclusion: String },
    WorkerStatus { worker_id: Uuid, status: String },
    WorkerComplete { worker_id: Uuid, result: String },
}
```

**`#[serde(untagged)]`** for response types where the format varies (e.g., an API that returns either a success object or an error object):

```rust
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApiResponse<T> {
    Ok(T),
    Err(ApiErrorResponse),
}
```

Order matters -- serde tries variants top to bottom, so put the most common case first.

**`#[serde(flatten)]`** for embedding extensible or provider-specific fields without wrapping them in a nested object:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
```

This keeps the wire format flat while allowing arbitrary additional fields to pass through.

## Database Patterns

Three embedded databases, each doing what it's best at. No server processes.

**SQLite for relational data** (conversations, memory graph, heartbeats):

```rust
pub struct MemoryStore {
    pool: SqlitePool,
}

impl MemoryStore {
    pub async fn connect(path: &Path) -> Result<Self> {
        let pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .context("failed to connect to memory database")?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn save(&self, memory: &Memory) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO memories (id, content, memory_type, importance, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
            memory.id,
            memory.content,
            memory.memory_type,
            memory.importance,
            memory.created_at,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
```

**LanceDB for vector/search data** (embeddings, full-text index):

Queries live in the modules that use them, not in a monolithic repository. `memory/store.rs` has graph queries, `memory/lance.rs` has embedding storage and search, `conversation/history.rs` has conversation queries.

**redb for key-value config** (settings, encrypted secrets):

Separate from SQLite so config can be backed up or moved independently.

**Fire-and-forget persistence** for non-critical writes:
```rust
let pool = self.pool.clone();
let message = message.clone();
tokio::spawn(async move {
    if let Err(error) = save_message(&pool, &message).await {
        tracing::warn!(%error, "failed to persist message");
    }
});
```

## Rig Integration Patterns

Every LLM process is a Rig `Agent`. They differ in system prompt, tools, history, and hooks.

**Agent construction follows a standard pattern:**
```rust
let agent = AgentBuilder::new(model.clone())
    .preamble(&system_prompt)
    .hook(SpacebotHook::new(process_id, process_type, event_tx.clone()))
    .tool_server_handle(tools.clone())
    .default_max_turns(50)
    .build();
```

**History is external, passed on each call:**
```rust
// Non-streaming: borrows history mutably, appends in-place
let response = agent.prompt(&user_message)
    .with_history(&mut history)
    .max_turns(5)
    .await?;

// Branching is a clone of history
let branch_history = channel_history.clone();
```

**Handle Rig's error types for recovery:**
```rust
match result {
    Err(PromptError::MaxTurnsError { chat_history, .. }) => {
        // Worker hit turn limit. Save progress, report partial result.
    }
    Err(PromptError::PromptCancelled { chat_history, reason }) => {
        // Hook terminated the loop (budget, cancellation, timeout).
    }
    Err(other) => return Err(other.into()),
    Ok(response) => { ... }
}
```

**PromptHook implementations** observe and report. They rarely modify behavior except for budget/cancellation:
```rust
#[derive(Clone)]
pub struct SpacebotHook {
    process_id: Uuid,
    process_type: ProcessType,
    event_tx: mpsc::Sender<ProcessEvent>,
}

impl PromptHook<SpacebotModel> for SpacebotHook {
    async fn on_tool_call(&self, tool_name: &str, ..) -> ToolCallHookAction {
        self.event_tx.send(ProcessEvent::ToolStarted {
            tool_name: tool_name.to_string(),
        }).ok();
        ToolCallHookAction::Continue
    }
}
```

**Tool definitions use doc comments as LLM instructions:**
```rust
/// Search the user's memories for relevant context.
///
/// Use this tool when you need to recall information from past conversations,
/// stored facts, preferences, or decisions.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MemoryRecallInput {
    /// The search query. Be specific -- include key terms the memory
    /// might contain rather than abstract descriptions.
    pub query: String,

    /// Maximum number of results to return.
    #[serde(default = "default_recall_limit")]
    pub limit: usize,
}
```

The doc comments on input structs and their fields serve dual purpose: Rust documentation AND instructions to the LLM.

## Logging and Tracing

Use the `tracing` crate. Structure log fields as key-value pairs.

```rust
tracing::info!(channel_id = %self.id, "channel started");
tracing::debug!(worker_id = %id, status = ?new_status, "worker status changed");
tracing::warn!(%error, memory_id = %id, "failed to persist memory");
tracing::error!(%error, "compaction worker crashed");
```

**Use `#[instrument]` for function-level spans:**
```rust
#[tracing::instrument(skip(self, memory_store), fields(channel_id = %self.id))]
async fn handle_message(&mut self, message: UserMessage, memory_store: &MemoryStore) -> Result<()> {
    // ...
}
```

Skip large/non-Debug parameters. Include identifying fields.

**Log levels:**
- `error` -- something is broken and needs attention
- `warn` -- something failed but the system can continue (e.g., failed background persistence)
- `info` -- significant lifecycle events (channel started, worker spawned, compaction triggered)
- `debug` -- detailed operational info (status changes, tool calls, branch results)
- `trace` -- very verbose (full message contents, raw LLM responses)

## Security Patterns

**Secrets use a wrapper that prevents accidental logging:**
```rust
pub struct DecryptedSecret(String);

impl DecryptedSecret {
    pub fn expose(&self) -> &str { &self.0 }
}

impl std::fmt::Debug for DecryptedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DecryptedSecret(***)")
    }
}

impl std::fmt::Display for DecryptedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "***")
    }
}
```

**Worker tool output is scanned** before entering LLM context. Use `SpacebotHook::on_tool_result()` for universal scan-after-execution.

**File tools reject writes** to identity/memory paths with an error directing the LLM to the correct tool.

## String Handling

- **`Arc<str>`** for immutable string IDs shared across tasks: `pub type ChannelId = Arc<str>;`
- **`String`** for owned mutable data
- **`&str`** for borrowed references
- **`format!`** for string construction, **`write!`** for appending to existing buffers
- **`.into()`** for conversions where the target type is unambiguous

**`impl Into<String>` for constructors** that take owned string data. This lets callers pass `&str`, `String`, or anything else that converts, without forcing `.to_string()` at every call site:

```rust
pub fn new(title: impl Into<String>, task: impl Into<String>) -> Self {
    Self {
        title: title.into(),
        task: task.into(),
        ..Default::default()
    }
}

// Callers can pass either:
Worker::new("compile", "run cargo build")
Worker::new(format!("task-{id}"), task_description)
```

Use `&str` for parameters that don't need ownership (search queries, lookups). Use `impl Into<String>` for parameters that will be stored.

## Iterators

**Chained iterator methods** are the preferred way to transform collections:
```rust
let active_workers = self.workers
    .values()
    .filter(|worker| worker.is_active())
    .map(|worker| worker.status_summary())
    .collect::<Vec<_>>();
```

**Turbofish on `.collect()`** to specify the target type:
```rust
.collect::<Vec<_>>()
.collect::<String>()
.collect::<HashMap<_, _>>()
```

**`futures::future::join_all` for parallel async:**
```rust
let results = futures::future::join_all(
    tasks.iter().map(|task| task.execute())
).await;
```

## State Machines

Use enums with data-carrying variants. Avoid separate structs for each state.

```rust
#[derive(Debug, Clone)]
pub enum WorkerState {
    Running,
    WaitingForInput { prompt: String },
    Done { result: String },
    Failed { error: String },
}
```

**Transition validation** declares valid transitions as data using `matches!`:

```rust
impl WorkerState {
    pub fn can_transition_to(&self, target: &WorkerState) -> bool {
        use WorkerState::*;
        matches!(
            (self, target),
            (Running, WaitingForInput { .. }) |
            (Running, Done { .. }) |
            (Running, Failed { .. }) |
            (WaitingForInput { .. }, Running) |
            (WaitingForInput { .. }, Failed { .. })
        )
    }

    pub fn transition_to(&mut self, new_state: WorkerState) -> Result<()> {
        if !self.can_transition_to(&new_state) {
            anyhow::bail!(
                "can't transition from {self:?} to {new_state:?}"
            );
        }
        *self = new_state;
        Ok(())
    }
}
```

This keeps the transition rules readable and makes illegal state transitions a runtime error instead of a silent bug.

## Constants

**Module-level constants for thresholds and limits:**
```rust
pub const MAX_CONCURRENT_BRANCHES: usize = 5;
pub const COMPACTION_THRESHOLD: f32 = 0.80;
pub const AGGRESSIVE_COMPACTION_THRESHOLD: f32 = 0.85;
pub const EMERGENCY_TRUNCATION_THRESHOLD: f32 = 0.95;
pub(crate) const MAX_RETRY_ATTEMPTS: u8 = 3;
pub(crate) const BASE_RETRY_DELAY: Duration = Duration::from_secs(5);
```

**Associated constants on types:**
```rust
impl Memory {
    pub const IDENTITY_IMPORTANCE: f32 = 1.0;
    pub const DEFAULT_IMPORTANCE: f32 = 0.5;
}
```

**`LazyLock` for complex static initialization:**
```rust
static LEAK_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"sk-[a-zA-Z0-9]{48}").expect("hardcoded regex"),
        Regex::new(r"-----BEGIN.*PRIVATE KEY-----").expect("hardcoded regex"),
    ]
});
```

## Panics

Avoid functions that panic. Prefer `?` for error propagation.

- Never use `.unwrap()` on `Result` or `Option` in production code
- Use `debug_assert!` for invariant checks in hot paths
- Prefer `.get()` or iterators over `collection[index]`
- `.expect("description")` is acceptable only when the invariant is truly guaranteed by construction (e.g., hardcoded regex compilation, infallible conversions)

## Unsafe

Don't use `unsafe`. If you think you need it, you probably don't. If you actually do, discuss it first.

## Testing

**Test module placement:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_roundtrip() {
        let store = MemoryStore::connect_in_memory().await.unwrap();
        let memory = Memory::new(MemoryType::Fact, "test fact");
        store.save(&memory).await.unwrap();

        let loaded = store.load(&memory.id).await.unwrap().unwrap();
        assert_eq!(loaded.content, "test fact");
    }

    #[test]
    fn test_status_block_formatting() {
        let block = StatusBlock::new();
        assert!(block.render().is_empty());
    }
}
```

Note: `.unwrap()` is acceptable in tests.

**Common test setup helper:**
```rust
async fn setup_test_channel() -> (Channel, mpsc::Receiver<ProcessEvent>) {
    let (event_tx, event_rx) = mpsc::channel(64);
    let memory_store = Arc::new(MemoryStore::connect_in_memory().await.unwrap());
    let channel = Channel::new(
        ChannelId::from("test"),
        memory_store,
        event_tx,
    );
    (channel, event_rx)
}
```

**`indoc!` for multiline test fixtures:**
```rust
let prompt = indoc! {"
    You are a memory recall worker.
    Search for relevant memories and return the top results.
"};
```

**Assertion patterns:**
```rust
assert_eq!(memories.len(), 3);
assert!(matches!(status, WorkerState::Done { .. }));
assert!(result.contains("auth module"), "expected auth-related result, got: {result}");
```

## `..Default::default()` for Partial Initialization

```rust
let config = HeartbeatConfig {
    prompt: "Check inbox".into(),
    interval_secs: 1800,
    ..Default::default()
};
```

## Prompts Are Files

System prompts live in `prompts/` as markdown files, not as string constants in Rust. Load them at startup or on demand. This makes them editable without recompilation.

```rust
let channel_prompt = tokio::fs::read_to_string("prompts/channel.md")
    .await
    .context("failed to load channel prompt")?;
```

Identity files (`SOUL.md`, `IDENTITY.md`, `USER.md`) are loaded by the `identity/` module and injected into system prompts.

## Graceful Shutdown

All long-running loops should respect a shutdown signal:

```rust
let mut shutdown_rx = shutdown_tx.subscribe();

loop {
    tokio::select! {
        Some(message) = message_rx.recv() => {
            self.handle_message(message).await?;
        }
        _ = shutdown_rx.recv() => {
            tracing::info!(channel_id = %self.id, "channel shutting down");
            break;
        }
    }
}
```

Workers, channels, heartbeat loops, and the cortex all participate in coordinated shutdown.