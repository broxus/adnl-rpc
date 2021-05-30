use std::cmp::Ordering;
use std::str::FromStr;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use ton_types::UInt256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetContractState {
    #[serde(with = "serde_address")]
    pub address: ton_block::MsgAddressInt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessage {
    #[serde(with = "serde_ton_block")]
    pub message: ton_block::Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTransactions {
    #[serde(with = "serde_address")]
    pub address: ton_block::MsgAddressInt,
    pub transaction_id: Option<TransactionId>,
    pub count: u8,
}

#[derive(Debug, Copy, Clone, Eq, Serialize, Deserialize)]
pub struct TransactionId {
    #[serde(with = "serde_u64")]
    pub lt: u64,
    #[serde(with = "serde_uint256")]
    pub hash: UInt256,
}

impl PartialEq for TransactionId {
    fn eq(&self, other: &Self) -> bool {
        self.lt == other.lt
    }
}

impl PartialOrd for TransactionId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TransactionId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.lt.cmp(&other.lt)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "messageType", content = "payload")]
pub enum WsRequestMessage {
    #[serde(rename_all = "camelCase")]
    SubscribeAccount {
        #[serde(with = "serde_address")]
        address: ton_block::MsgAddressInt,
    },
    #[serde(rename_all = "camelCase")]
    SubscribeForNewBlock,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "messageType", content = "payload")]
pub enum WsResponseMessage {
    Transaction(serde_json::Value),
    Block {},
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum RawContractState {
    NotExists,
    Exists(ExistingContract),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingContract {
    #[serde(with = "serde_ton_block")]
    pub account: ton_block::AccountStuff,
    pub timings: GenTimings,
    pub last_transaction_id: TransactionId,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct GenTimings {
    #[serde(with = "serde_u64")]
    pub gen_lt: u64,
    pub gen_utime: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTransactionsList {
    #[serde(with = "serde_bytes")]
    pub transactions: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawBlock {
    #[serde(with = "serde_ton_block")]
    pub block: ton_block::Block,
}

pub mod serde_u64 {
    use serde::de::Error;
    use serde::Deserialize;

    use super::*;

    pub fn serialize<S>(data: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        data.to_string().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|data| u64::from_str(&data).map_err(D::Error::custom))
    }
}

pub mod serde_uint256 {
    use serde::de::Error;
    use serde::Deserialize;

    use super::*;

    pub fn serialize<S>(data: &UInt256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&data.to_hex_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<UInt256, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = String::deserialize(deserializer)?;
        UInt256::from_str(&data).map_err(|_| D::Error::custom("Invalid uint256"))
    }
}

pub mod serde_address {
    use std::str::FromStr;

    use serde::de::Error;
    use serde::Deserialize;

    use super::*;

    pub fn serialize<S>(data: &ton_block::MsgAddressInt, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&data.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ton_block::MsgAddressInt, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = String::deserialize(deserializer)?;
        ton_block::MsgAddressInt::from_str(&data).map_err(|_| D::Error::custom("Invalid address"))
    }
}

pub mod serde_optional_address {
    use serde::{Deserialize, Serialize};

    use super::*;

    pub fn serialize<S>(
        data: &Option<ton_block::MsgAddressInt>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(transparent)]
        struct Wrapper<'a>(#[serde(with = "serde_address")] &'a ton_block::MsgAddressInt);

        match data {
            Some(data) => serializer.serialize_some(&Wrapper(data)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<ton_block::MsgAddressInt>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(transparent)]
        struct Wrapper(#[serde(with = "serde_address")] ton_block::MsgAddressInt);

        Option::<Wrapper>::deserialize(deserializer).map(|wrapper| wrapper.map(|data| data.0))
    }
}

pub mod serde_message {
    use super::*;
    use ton_block::{Deserializable, Serializable};

    pub fn serialize<S>(data: &ton_block::Message, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::Error;

        serde_cell::serialize(&data.serialize().map_err(S::Error::custom)?, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ton_block::Message, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let data = String::deserialize(deserializer)?;
        ton_block::Message::construct_from_base64(&data).map_err(D::Error::custom)
    }
}

pub mod serde_ton_block {
    use super::*;
    use ton_block::{Deserializable, Serializable};

    pub fn serialize<S, T>(data: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: Serializable,
    {
        use serde::ser::Error;

        serde_cell::serialize(&data.serialize().map_err(S::Error::custom)?, serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: Deserializable,
    {
        use serde::de::Error;

        let data = String::deserialize(deserializer)?;
        T::construct_from_base64(&data).map_err(D::Error::custom)
    }
}

pub mod serde_boc {
    use super::*;

    pub fn serialize<S>(data: &ton_types::SliceData, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde_cell::serialize(&data.into_cell(), serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ton_types::SliceData, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde_cell::deserialize(deserializer).map(From::from)
    }
}

pub mod serde_cell {
    use serde::de::Deserialize;
    use ton_types::Cell;

    pub fn serialize<S>(data: &Cell, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        use serde::ser::Error;

        let bytes = ton_types::serialize_toc(data).map_err(S::Error::custom)?;
        serializer.serialize_str(&base64::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Cell, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let data = String::deserialize(deserializer)?;
        let bytes = base64::decode(&data).map_err(D::Error::custom)?;
        let cell = ton_types::deserialize_tree_of_cells(&mut std::io::Cursor::new(&bytes))
            .map_err(D::Error::custom)?;
        Ok(cell)
    }
}

pub mod serde_bytes {
    use super::*;

    pub fn serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        base64::encode(data).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let data = String::deserialize(deserializer)?;
        base64::decode(&data).map_err(D::Error::custom)
    }
}
