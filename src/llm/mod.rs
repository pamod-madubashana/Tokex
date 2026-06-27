//! LLM integration: compression insights and MCP server.

pub mod compress;
pub mod mcp;

pub use compress::{compress, LlmConfig};
