//! AI module for test annotation
//!
//! Provides:
//! - Automatic test annotation using AI
//! - Support for multiple AI providers (Anthropic, OpenAI)
//! - Code analysis and description generation

mod annotator;
mod providers;

pub use annotator::TestAnnotator;
pub use providers::{AiClient, AiConfig, AiProvider, TestAnnotation};
