//! Test harness for CLI integration tests.
//!
//! Provides isolated test environments, programmatic note creation,
//! and CLI assertion helpers using `assert_cmd`.

mod command;
mod env;
mod note;

// Re-export main types for external use
#[allow(unused_imports)]
pub use command::DenCommand;
#[allow(unused_imports)]
pub use env::TestEnv;
#[allow(unused_imports)]
pub use note::TestNote;
