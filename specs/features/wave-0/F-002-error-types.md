# F-002: Error Types and Runtime Trait Boundary

## 1. Overview

**Module:** error, runtime
**Priority:** P0
**Depends on:** F-001

Define the error type hierarchy (`BundleError` enum) and the AmplifierRuntime trait boundary. These are foundational types that every other module depends on. After this feature, `src/error.rs` and `src/runtime.rs` are complete.

## 2. Requirements

### Interfaces — Error Types (src/error.rs)

```rust
use std::fmt;

/// Validation result carrying errors and warnings.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} errors, {} warnings", self.errors.len(), self.warnings.len())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("bundle not found: {uri}")]
    NotFound { uri: String },

    #[error("failed to load bundle: {reason}")]
    LoadError {
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("validation failed: {0}")]
    ValidationError(ValidationResult),

    #[error("dependency error: {0}")]
    DependencyError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("http error: {0}")]
    Http(String),

    #[error("git error: {0}")]
    Git(String),
}

pub type Result<T> = std::result::Result<T, BundleError>;
```

### Interfaces — Runtime Traits (src/runtime.rs)

Port the full 14-interaction-point trait boundary from the architecture spec section 6. All traits must compile and be usable with mockall.

```rust
use async_trait::async_trait;
use futures::future::BoxFuture;
use std::any::Any;

use crate::error::Result;

pub struct SessionOptions {
    pub mount_plan: serde_yaml_ng::Value,
    pub session_id: Option<String>,
    pub parent_id: Option<String>,
    pub approval_system: Option<Box<dyn ApprovalSystem>>,
    pub display_system: Option<Box<dyn DisplaySystem>>,
    pub is_resumed: bool,
}

#[async_trait]
pub trait AmplifierRuntime: Send + Sync {
    async fn create_session(&self, opts: SessionOptions) -> Result<Box<dyn AmplifierSession>>;
}

#[async_trait]
pub trait AmplifierSession: Send + Sync {
    fn session_id(&self) -> &str;
    fn coordinator(&self) -> &dyn Coordinator;
    fn coordinator_mut(&mut self) -> &mut dyn Coordinator;
    async fn initialize(&mut self) -> Result<()>;
    async fn execute(&mut self, instruction: &str) -> Result<String>;
    async fn cleanup(&mut self) -> Result<()>;
}

pub trait Coordinator: Send + Sync {
    fn mount(&mut self, name: &str, component: Box<dyn Any + Send + Sync>);
    fn get(&self, name: &str) -> Option<&(dyn Any + Send + Sync)>;
    fn register_capability(&mut self, key: &str, value: serde_json::Value);
    fn get_capability(&self, key: &str) -> Option<&serde_json::Value>;
    fn approval_system(&self) -> Option<&dyn ApprovalSystem>;
    fn display_system(&self) -> Option<&dyn DisplaySystem>;
    fn hooks(&self) -> &dyn HookRegistry;
    fn hooks_mut(&mut self) -> &mut dyn HookRegistry;
}

pub trait HookRegistry: Send + Sync {
    fn register(&mut self, event: &str, handler: Box<dyn HookHandler>, priority: i32, name: &str);
}

pub trait ContextManager: Send + Sync {
    fn set_system_prompt_factory(&mut self, factory: Box<dyn SystemPromptFactory>);
    fn set_messages(&mut self, messages: Vec<serde_json::Value>);
    fn add_message(&mut self, message: serde_json::Value);
}

pub trait ApprovalSystem: Send + Sync {}
pub trait DisplaySystem: Send + Sync {}
pub trait HookHandler: Send + Sync {}

pub trait SystemPromptFactory: Send + Sync {
    fn create(&self) -> BoxFuture<'_, String>;
}
```

### Behavior

- `BundleError` matches Python's exception hierarchy 1:1:
  - `BundleError` (base) -> `BundleError` enum (the enum itself)
  - `BundleNotFoundError` -> `BundleError::NotFound`
  - `BundleLoadError` -> `BundleError::LoadError`
  - `BundleValidationError` -> `BundleError::ValidationError`
  - `BundleDependencyError` -> `BundleError::DependencyError`
- `Http` variant uses String (not `#[from] reqwest::Error`) to avoid requiring reqwest when http-sources feature is disabled
- All runtime traits must be object-safe (usable as `dyn Trait`)
- `SessionOptions` fields use `Option<Box<dyn Trait>>` for optional systems

## 3. Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-1 | `BundleError` enum compiles with all 8 variants | `cargo check` |
| AC-2 | `ValidationResult` is `Debug + Clone + Display` | Compilation |
| AC-3 | `Result<T>` type alias works throughout crate | Compilation |
| AC-4 | All 7 runtime traits compile as object-safe | `cargo check` |
| AC-5 | `BundleError` implements `std::error::Error + Send + Sync` | Compilation |
| AC-6 | `From<std::io::Error>` and `From<serde_yaml_ng::Error>` work | Compilation |

## 4. Edge Cases

| Case | Expected Behavior |
|------|-------------------|
| `BundleError::LoadError` with no source | `source: None` is valid |
| `ValidationResult` with empty errors | Display shows "0 errors, 0 warnings" |
| `Http` variant without reqwest feature | Compiles — uses String, not reqwest::Error |

## 5. Files to Create/Modify

| File | Action | Contents |
|------|--------|----------|
| `src/error.rs` | Modify | BundleError enum, ValidationResult, Result alias |
| `src/runtime.rs` | Modify | All 7 traits + SessionOptions struct |
| `src/lib.rs` | Modify | Ensure `pub mod error; pub mod runtime;` present |

## 6. Dependencies

No new dependencies. Uses: thiserror, serde_yaml_ng, serde_json, async-trait, futures (all in Cargo.toml from F-001).

## 7. Notes

- `Http` variant deliberately does NOT use `#[from] reqwest::Error` because reqwest is optional. Code that catches reqwest errors should convert: `reqwest_err.to_string()` into `BundleError::Http(msg)`.
- The runtime traits are NOT implemented in this feature — only defined. Mock implementations come later.
- `SystemPromptFactory::create` returns `BoxFuture<'_, String>` — this is the `PreparedBundle` async closure pattern. The trait definition is straightforward; the implementation (Wave 3) is the hard part.
