use crate::auction::{Bid, SignedBid};
use crate::error::{AlgorandError, ApiError};
use crate::transaction::{SignedTransaction, Transaction};
use algonaut_core::{
    Address, LogicSignature, MultisigAddress, MultisigSignature, MultisigSubsig, Signature,
    ToMsgPack,
};
use algonaut_crypto::mnemonic;
use algonaut_crypto::Ed25519PublicKey;
use data_encoding::BASE32_NOPAD;
use rand::rngs::OsRng;
use rand::Rng;
use ring::signature::Ed25519KeyPair as KeyPairType;
use ring::signature::KeyPair;
use sha2::Digest;
use std::borrow::Borrow;

type ChecksumAlg = sha2::Sha512Trunc256;

pub struct Account {
    seed: [u8; 32],
    address: Address,
    key_pair: KeyPairType,
}

impl Account {
    pub fn generate() -> Account {
        let seed: [u8; 32] = OsRng.gen();
        Self::from_seed(seed)
    }

    /// Create account from human readable mnemonic of a 32 byte seed
    pub fn from_mnemonic(mnemonic: &str) -> Result<Account, AlgorandError> {
        let seed = mnemonic::to_key(mnemonic)?;
        Ok(Self::from_seed(seed))
    }

    /// Create account from 32 byte seed
    pub fn from_seed(seed: [u8; 32]) -> Account {
        let key_pair = KeyPairType::from_seed_unchecked(&seed).unwrap();
        let mut pk = [0; 32];
        pk.copy_from_slice(key_pair.public_key().as_ref());
        let address = Address::new(pk);
        Account {
            seed,
            address,
            key_pair,
        }
    }

    /// Get the public key address of the account
    pub fn address(&self) -> Address {
        self.address
    }

    /// Get the human readable mnemonic of the 32 byte seed
    pub fn mnemonic(&self) -> String {
        mnemonic::from_key(&self.seed).unwrap()
    }

    /// Get the 32 byte seed
    pub fn seed(&self) -> [u8; 32] {
        self.seed
    }

    fn sign(&self, bytes: &[u8]) -> Signature {
        let signature = self.key_pair.sign(&bytes);
        // ring returns a signature with padding at the end to make it 105 bytes, only 64 bytes are actually used
        let mut stripped_signature = [0; 64];
        stripped_signature.copy_from_slice(&signature.as_ref()[..64]);
        Signature(stripped_signature)
    }

    pub fn sign_program(&self, bytes: &[u8]) -> Signature {
        self.sign(&["Program".as_bytes(), &bytes].concat())
    }

    /// Sign a bid with the account's private key
    pub fn sign_bid(&self, bid: Bid) -> Result<SignedBid, AlgorandError> {
        let encoded_bid = bid.to_msg_pack()?;
        let mut prefix_encoded_bid = b"aB".to_vec();
        prefix_encoded_bid.extend_from_slice(&encoded_bid);
        let signature = self.sign(&prefix_encoded_bid);
        Ok(SignedBid {
            bid,
            sig: signature,
        })
    }

    /// Sign a transaction with the account's private key
    pub fn sign_transaction(
        &self,
        transaction: &Transaction,
    ) -> Result<SignedTransaction, AlgorandError> {
        let transaction_bytes = &transaction.bytes_to_sign()?;
        let signature = self.sign(&transaction_bytes);
        let id = BASE32_NOPAD.encode(&ChecksumAlg::digest(&transaction.bytes_to_sign()?));
        Ok(SignedTransaction {
            transaction: transaction.clone(),
            sig: Some(signature),
            logicsig: None,
            multisig: None,
            transaction_id: id,
        })
    }

    pub fn sign_logic_msig(
        &self,
        lsig: LogicSignature,
        ma: MultisigAddress,
    ) -> Result<LogicSignature, AlgorandError> {
        let mut lsig = lsig;
        let my_public_key = Ed25519PublicKey(self.address.0);
        if !ma.public_keys.contains(&my_public_key) {
            return Err(ApiError::InvalidSecretKeyInMultisig.into());
        }
        let sig = self.sign_program(&lsig.logic);
        let subsigs: Vec<MultisigSubsig> = ma
            .public_keys
            .iter()
            .map(|key| {
                if *key == my_public_key {
                    MultisigSubsig {
                        key: *key,
                        sig: Some(sig),
                    }
                } else {
                    MultisigSubsig {
                        key: *key,
                        sig: None,
                    }
                }
            })
            .collect();
        let multisig = MultisigSignature {
            version: ma.version,
            threshold: ma.threshold,
            subsigs,
        };
        lsig.msig = Some(multisig);
        Ok(lsig)
    }

    pub fn append_to_logic_msig(
        &self,
        lsig: LogicSignature,
    ) -> Result<LogicSignature, AlgorandError> {
        let mut lsig = lsig;
        let my_public_key = Ed25519PublicKey(self.address.0);
        let msig = lsig
            .msig
            .ok_or_else(|| AlgorandError::from(ApiError::InvalidSecretKeyInMultisig))?;
        let mut found_my_public_key = false;
        let sig = self.sign_program(&lsig.logic);
        let subsigs: Vec<MultisigSubsig> = msig
            .subsigs
            .iter()
            .map(|subsig| {
                if subsig.key == my_public_key {
                    found_my_public_key = true;
                    MultisigSubsig {
                        key: subsig.key,
                        sig: Some(sig),
                    }
                } else {
                    MultisigSubsig {
                        key: subsig.key,
                        sig: subsig.sig,
                    }
                }
            })
            .collect();
        if !found_my_public_key {
            return Err(ApiError::InvalidSecretKeyInMultisig.into());
        }
        lsig.msig = Some(MultisigSignature { subsigs, ..msig });
        Ok(lsig)
    }

    /// Sign the transaction and populate the multisig field of the signed transaction with the given multisig address
    pub fn sign_multisig_transaction(
        &self,
        from: MultisigAddress,
        transaction: &Transaction,
    ) -> Result<SignedTransaction, AlgorandError> {
        if from.address() != transaction.sender {
            return Err(ApiError::InvalidSenderInMultisig.into());
        }
        let my_public_key = Ed25519PublicKey(self.address.0);
        if !from.public_keys.contains(&my_public_key) {
            return Err(ApiError::InvalidSecretKeyInMultisig.into());
        }
        let signed_transaction = self.sign_transaction(transaction)?;
        let subsigs: Vec<MultisigSubsig> = from
            .public_keys
            .iter()
            .map(|key| {
                if *key == my_public_key {
                    MultisigSubsig {
                        key: *key,
                        sig: signed_transaction.clone().sig,
                    }
                } else {
                    MultisigSubsig {
                        key: *key,
                        sig: None,
                    }
                }
            })
            .collect();
        let multisig = MultisigSignature {
            version: from.version,
            threshold: from.threshold,
            subsigs,
        };
        Ok(SignedTransaction {
            multisig: Some(multisig),
            sig: None,
            logicsig: None,
            transaction: transaction.clone(),
            transaction_id: signed_transaction.transaction_id,
        })
    }

    /// Appends the multisig signature from the given multisig address to the transaction
    pub fn append_multisig_transaction(
        &self,
        from: MultisigAddress,
        transaction: &SignedTransaction,
    ) -> Result<SignedTransaction, AlgorandError> {
        let from_transaction = self.sign_multisig_transaction(from, &transaction.transaction)?;
        Self::merge_multisig_transactions(&[&from_transaction, transaction])
    }

    /// Returns a signed transaction with the multisig signatures of the passed signed transactions merged
    pub fn merge_multisig_transactions<T: Borrow<SignedTransaction>>(
        transactions: &[T],
    ) -> Result<SignedTransaction, AlgorandError> {
        if transactions.len() < 2 {
            return Err(ApiError::InsufficientTransactions.into());
        }
        let mut merged = transactions[0].borrow().clone();
        for transaction in transactions {
            let merged_msig = merged.multisig.as_mut().unwrap();
            let msig = transaction.borrow().multisig.as_ref().unwrap();
            if merged_msig.subsigs.len() != msig.subsigs.len() {
                return Err(ApiError::InvalidNumberOfSubsignatures.into());
            }
            assert_eq!(merged_msig.subsigs.len(), msig.subsigs.len());
            for (merged_subsig, subsig) in merged_msig.subsigs.iter_mut().zip(&msig.subsigs) {
                if subsig.key != merged_subsig.key {
                    return Err(ApiError::InvalidPublicKeyInMultisig.into());
                }
                if merged_subsig.sig.is_none() {
                    merged_subsig.sig = subsig.sig
                } else if merged_subsig.sig != subsig.sig && subsig.sig.is_some() {
                    return Err(ApiError::MismatchingSignatures.into());
                }
            }
        }
        Ok(merged)
    }
}
