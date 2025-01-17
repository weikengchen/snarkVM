// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::{traits::TransactionScheme, TransactionError};
use snarkvm_utilities::{
    bytes::{FromBytes, ToBytes},
    has_duplicates,
    to_bytes,
    variable_length_integer::{read_variable_length_integer, variable_length_integer},
};

use std::{
    io::{Read, Result as IoResult, Write},
    ops::{Deref, DerefMut},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Transactions<T: TransactionScheme>(pub Vec<T>);

impl<T: TransactionScheme> Transactions<T> {
    /// Initializes an empty list of transactions.
    pub fn new() -> Self {
        Self(vec![])
    }

    /// Initializes from a given list of transactions.
    pub fn from(transactions: &[T]) -> Self {
        Self(transactions.to_vec())
    }

    /// Initializes an empty list of transactions.
    pub fn push(&mut self, transaction: T) {
        self.0.push(transaction);
    }

    /// Returns the transaction ids.
    pub fn to_transaction_ids(&self) -> Result<Vec<[u8; 32]>, TransactionError> {
        self.0.iter().map(|tx| tx.transaction_id()).collect()
    }

    /// Serializes the transactions into byte vectors.
    pub fn serialize(&self) -> Result<Vec<Vec<u8>>, TransactionError> {
        self.0
            .iter()
            .map(|transaction| -> Result<Vec<u8>, TransactionError> { Ok(to_bytes![transaction]?) })
            .collect::<Result<Vec<Vec<u8>>, TransactionError>>()
    }

    /// Serializes the transactions into strings.
    pub fn serialize_as_str(&self) -> Result<Vec<String>, TransactionError> {
        self.0
            .iter()
            .map(|transaction| -> Result<String, TransactionError> { Ok(hex::encode(to_bytes![transaction]?)) })
            .collect::<Result<Vec<String>, TransactionError>>()
    }

    pub fn conflicts(&self, transaction: &T) -> bool {
        let mut holding_serial_numbers = vec![];
        let mut holding_commitments = vec![];
        let mut holding_memos = Vec::with_capacity(self.0.len());

        for tx in &self.0 {
            if tx.network_id() != transaction.network_id() {
                return true;
            };

            holding_serial_numbers.extend(tx.old_serial_numbers());
            holding_commitments.extend(tx.new_commitments());
            holding_memos.push(tx.memorandum());
        }

        let transaction_serial_numbers = transaction.old_serial_numbers();
        let transaction_commitments = transaction.new_commitments();
        let transaction_memo = transaction.memorandum();

        // Check if the transactions in the block have duplicate serial numbers
        if has_duplicates(transaction_serial_numbers) {
            return true;
        }

        // Check if the transactions in the block have duplicate commitments
        if has_duplicates(transaction_commitments) {
            return true;
        }

        if holding_memos.contains(&transaction_memo) {
            return true;
        }

        for sn in transaction_serial_numbers {
            if holding_serial_numbers.contains(&sn) {
                return true;
            }
        }

        for cm in transaction_commitments {
            if holding_commitments.contains(&cm) {
                return true;
            }
        }

        false
    }
}

impl<T: TransactionScheme> ToBytes for Transactions<T> {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        variable_length_integer(self.0.len() as u64).write(&mut writer)?;

        for transaction in &self.0 {
            transaction.write(&mut writer)?;
        }

        Ok(())
    }
}

impl<T: TransactionScheme> FromBytes for Transactions<T> {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let num_transactions = read_variable_length_integer(&mut reader)?;
        let mut transactions = Vec::with_capacity(num_transactions);
        for _ in 0..num_transactions {
            let transaction: T = FromBytes::read(&mut reader)?;
            transactions.push(transaction);
        }

        Ok(Self(transactions))
    }
}

impl<T: TransactionScheme> Default for Transactions<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: TransactionScheme> Deref for Transactions<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: TransactionScheme> DerefMut for Transactions<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}
