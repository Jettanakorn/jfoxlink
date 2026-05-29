use crate::types::{Chunk, Message, ToolDef};
use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat(&self, messages: &[Message], tools: &[ToolDef]) -> anyhow::Result<Message>;
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
    ) -> anyhow::Result<mpsc::Receiver<Chunk>>;
}
