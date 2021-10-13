// Copyright (c) 2021 MASSA LABS <info@massa.net>

#[macro_use]
extern crate lazy_static;
pub use address::{Address, ADDRESS_SIZE_BYTES};
pub use amount::Amount;
pub use block::{
    Block, BlockHashMap, BlockHashSet, BlockHeader, BlockHeaderContent, BlockId,
    BLOCK_ID_SIZE_BYTES,
};
pub use composite::{
    OperationSearchResult, OperationSearchResultBlockStatus, OperationSearchResultStatus,
    StakersCycleProductionStats,
};
pub use context::{
    get_serialization_context, init_serialization_context, with_serialization_context,
    SerializationContext,
};
pub use endorsement::{
    Endorsement, EndorsementContent, EndorsementHashMap, EndorsementHashSet, EndorsementId,
};
pub use error::ModelsError;
pub use operation::{
    Operation, OperationContent, OperationHashMap, OperationHashSet, OperationId, OperationType,
    OPERATION_ID_SIZE_BYTES,
};
pub use serialization::{
    array_from_slice, u8_from_slice, DeserializeCompact, DeserializeMinBEInt, DeserializeVarInt,
    SerializeCompact, SerializeMinBEInt, SerializeVarInt,
};
pub use slot::{Slot, SLOT_KEY_SIZE};
pub use version::Version;

pub mod address;
pub mod amount;
mod block;
pub mod clique;
mod composite;
mod context;
pub mod crypto;
mod endorsement;
pub mod error;
pub mod hhasher;
pub mod ledger;
pub mod node;
pub mod operation;
mod serialization;
pub mod slot;
pub mod timeslots;
mod version;
