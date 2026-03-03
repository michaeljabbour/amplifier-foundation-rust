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
