use std::prelude::v1::*;

extern crate derive_more;
use derive_more::{Display, From};
use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Display, Error, From, Clone)]
pub enum CryptoError {
    #[display(fmt = "Key length is invalid.")]
    InvalidKeyLength,
    #[display(fmt = "Mnemonic length is invalid.")]
    InvalidMnemonicLength,
    #[display(fmt = "Mnemonic contains invalid words.")]
    InvalidWordsInMnemonic,
    #[display(fmt = "Invalid checksum.")]
    InvalidChecksum,
}
