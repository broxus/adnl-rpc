use std::cmp::Ordering;
use std::convert::TryFrom;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use ton_block::MsgAddressInt;
use ton_types::UInt256;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Message {
    /// Source message address, `None` for external messages
    #[serde(with = "serde_optional_address")]
    pub src: Option<MsgAddressInt>,

    /// Destination message address, `None` for outbound messages
    #[serde(with = "serde_optional_address")]
    pub dst: Option<MsgAddressInt>,

    /// Message value in nano TON
    pub value: u64,

    /// Message body
    pub body: Option<MessageBody>,

    /// Whether this message will be bounced on unsuccessful execution.
    pub bounce: bool,

    /// Whether this message was bounced during unsuccessful execution.
    /// Only relevant for internal messages
    pub bounced: bool,
}

impl From<ton_block::Message> for Message {
    fn from(s: ton_block::Message) -> Self {
        let body = s.body().and_then(|body| MessageBody::try_from(body).ok());

        match s.header() {
            ton_block::CommonMsgInfo::IntMsgInfo(header) => Message {
                src: match &header.src {
                    ton_block::MsgAddressIntOrNone::Some(addr) => Some(addr.clone()),
                    ton_block::MsgAddressIntOrNone::None => None,
                },
                dst: Some(header.dst.clone()),
                value: header.value.grams.0 as u64,
                body,
                bounce: header.bounce,
                bounced: header.bounced,
            },
            ton_block::CommonMsgInfo::ExtInMsgInfo(header) => Message {
                src: None,
                dst: Some(header.dst.clone()),
                body,
                ..Default::default()
            },
            ton_block::CommonMsgInfo::ExtOutMsgInfo(header) => Message {
                src: match &header.src {
                    ton_block::MsgAddressIntOrNone::Some(addr) => Some(addr.clone()),
                    ton_block::MsgAddressIntOrNone::None => None,
                },
                body,
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageBody {
    /// Hash of body in cell representation
    #[serde(with = "serde_uint256")]
    pub hash: UInt256,
    /// Base64 encoded message body
    pub data: String,
}

impl MessageBody {
    pub fn decode(&self) -> Result<ton_types::Cell> {
        let bytes = base64::decode(&self.data)?;
        let cell = ton_types::deserialize_tree_of_cells(&mut std::io::Cursor::new(&bytes))
            .map_err(|_| MessageBodyError::FailedToDeserialize)?;
        Ok(cell)
    }
}

impl TryFrom<ton_types::SliceData> for MessageBody {
    type Error = MessageBodyError;

    fn try_from(s: ton_types::SliceData) -> Result<Self, Self::Error> {
        let cell = s.into_cell();
        let hash = cell.repr_hash();
        let bytes =
            ton_types::serialize_toc(&cell).map_err(|_| MessageBodyError::FailedToSerialize)?;
        Ok(Self {
            hash,
            data: base64::encode(bytes),
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MessageBodyError {
    #[error("Failed to serialize data")]
    FailedToSerialize,
    #[error("Failed to deserialize data")]
    FailedToDeserialize,
}

#[derive(Debug, Copy, Clone, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type", content = "data")]
pub enum LastTransactionId {
    Exact(TransactionId),
    Inexact { latest_lt: u64 },
}

impl LastTransactionId {
    /// Whether the exact id is known
    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    /// Converts last transaction id into real or fake id
    pub fn to_transaction_id(self) -> TransactionId {
        match self {
            Self::Exact(id) => id,
            Self::Inexact { latest_lt } => TransactionId {
                lt: latest_lt,
                hash: Default::default(),
            },
        }
    }
}

impl PartialEq for LastTransactionId {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Exact(left), Self::Exact(right)) => left == right,
            (Self::Inexact { latest_lt: left }, Self::Inexact { latest_lt: right }) => {
                left == right
            }
            _ => false,
        }
    }
}

impl PartialOrd for LastTransactionId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LastTransactionId {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = match self {
            Self::Exact(id) => &id.lt,
            Self::Inexact { latest_lt } => latest_lt,
        };
        let right = match other {
            Self::Exact(id) => &id.lt,
            Self::Inexact { latest_lt } => latest_lt,
        };
        left.cmp(right)
    }
}

#[derive(Debug, Copy, Clone, Eq, Serialize, Deserialize)]
pub struct TransactionId {
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

    pub fn serialize<S>(data: &MsgAddressInt, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&data.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<MsgAddressInt, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = String::deserialize(deserializer)?;
        MsgAddressInt::from_str(&data).map_err(|_| D::Error::custom("Invalid address"))
    }
}

pub mod serde_optional_address {
    use serde::{Deserialize, Serialize};

    use super::*;

    pub fn serialize<S>(data: &Option<MsgAddressInt>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(transparent)]
        struct Wrapper<'a>(#[serde(with = "serde_address")] &'a MsgAddressInt);

        match data {
            Some(data) => serializer.serialize_some(&Wrapper(data)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<MsgAddressInt>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(transparent)]
        struct Wrapper(#[serde(with = "serde_address")] MsgAddressInt);

        Option::<Wrapper>::deserialize(deserializer).map(|wrapper| wrapper.map(|data| data.0))
    }
}
