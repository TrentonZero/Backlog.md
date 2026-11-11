//! Internal Rust components for the staged Backlog.md executable migration.
pub mod config;
mod content_references;
pub mod decision_store;
pub mod discovery;
pub mod document_store;
pub mod draft_store;
pub mod frontmatter;
pub mod git;
pub mod identity;
pub mod records;
pub mod sections;
pub mod serialization;
pub mod storage;
pub mod task_file_ids;
pub mod task_store;
