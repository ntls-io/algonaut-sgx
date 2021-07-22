#![no_std]

//! # algonaut
//!
//! Rust **algonaut** aims at becoming a rusty SDK for [Algorand](https://www.algorand.com/).
//! Please, be aware that this crate is a work in progress.
//!
//! ## Objectives
//!
//! - Example-driven API development
//! - Async requests
//! - Builder pattern and sensible defaults
//! - Modularity
//! - Clear error messages
//! - Thorough test suite
//! - Comprehensive documentation

// TODO #![deny(missing_docs)]

// Re-exports

pub use algonaut_core as core;
pub use algonaut_crypto as crypto;
#[cfg(feature = "algonaut_model")] // SGX: not ported
pub use algonaut_model as model;
pub use algonaut_transaction as transaction;

#[cfg(feature = "algonaut_client")] // SGX: not ported
pub mod algod;
#[cfg(feature = "algonaut_client")] // SGX: not ported
pub mod error;
#[cfg(feature = "algonaut_client")] // SGX: not ported
pub mod indexer;
#[cfg(feature = "algonaut_client")] // SGX: not ported
pub mod kmd;
