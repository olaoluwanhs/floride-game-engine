//! Standard host-facing lifecycle contract for games in the workspace.
//!
//! This crate defines [`GameAdapter`], which enables any external host
//! (such as a command-line runner, WebAssembly browser runner, or engine wrapper)
//! to drive any game simulation uniformly.
//!
//! The contract uses associated types so each concrete game implementation retains
//! its own configuration, input vocabulary, snapshot structures, and event types
//! without forcing a universal command enum across different games.

mod adapter;

pub use adapter::GameAdapter;
