// Copyright (c) 2021 MASSA LABS <info@massa.net>

use massa_consensus::{BootstrapableGraph, ExportProofOfStake};
use massa_execution::BootstrapExecutionState;
use massa_hash::hash::Hash;
use massa_hash::HASH_SIZE_BYTES;
use massa_models::array_from_slice;
use massa_models::{
    DeserializeCompact, DeserializeVarInt, ModelsError, SerializeCompact, SerializeVarInt, Version,
};
use massa_network::BootstrapPeers;
use massa_time::MassaTime;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

/// Messages used during bootstrap
#[derive(Debug, Serialize, Deserialize)]
pub enum BootstrapMessage {
    /// Sync clocks
    BootstrapTime {
        /// The current time on the bootstrap server.
        server_time: MassaTime,
        version: Version,
        config_hash: Hash,
    },
    /// Bootstrap peers
    BootstrapPeers {
        /// Server peers
        peers: BootstrapPeers,
    },
    /// Consensus state
    ConsensusState {
        /// PoS
        pos: ExportProofOfStake,
        /// block graph
        graph: BootstrapableGraph,
    },
    /// Execution state
    ExecutionState {
        /// execution state
        execution_state: BootstrapExecutionState,
    },
}

#[derive(IntoPrimitive, Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u32)]
enum MessageTypeId {
    BootstrapTime = 0u32,
    Peers = 1u32,
    ConsensusState = 2u32,
    ExecutionState = 3u32,
}

impl SerializeCompact for BootstrapMessage {
    fn to_bytes_compact(&self) -> Result<Vec<u8>, ModelsError> {
        let mut res: Vec<u8> = Vec::new();
        match self {
            BootstrapMessage::BootstrapTime {
                server_time,
                version,
                config_hash,
            } => {
                res.extend(u32::from(MessageTypeId::BootstrapTime).to_varint_bytes());
                res.extend(server_time.to_bytes_compact()?);
                res.extend(&version.to_bytes_compact()?);
                res.extend(&config_hash.to_bytes())
            }
            BootstrapMessage::BootstrapPeers { peers } => {
                res.extend(u32::from(MessageTypeId::Peers).to_varint_bytes());
                res.extend(&peers.to_bytes_compact()?);
            }
            BootstrapMessage::ConsensusState { pos, graph } => {
                res.extend(u32::from(MessageTypeId::ConsensusState).to_varint_bytes());
                res.extend(&pos.to_bytes_compact()?);
                res.extend(&graph.to_bytes_compact()?);
            }
            BootstrapMessage::ExecutionState { execution_state } => {
                res.extend(u32::from(MessageTypeId::ExecutionState).to_varint_bytes());
                res.extend(&execution_state.to_bytes_compact()?);
            }
        }
        Ok(res)
    }
}

impl DeserializeCompact for BootstrapMessage {
    fn from_bytes_compact(buffer: &[u8]) -> Result<(Self, usize), ModelsError> {
        let mut cursor = 0usize;

        let (type_id_raw, delta) = u32::from_varint_bytes(&buffer[cursor..])?;
        cursor += delta;

        let type_id: MessageTypeId = type_id_raw
            .try_into()
            .map_err(|_| ModelsError::DeserializeError("invalid message type ID".into()))?;

        let res = match type_id {
            MessageTypeId::BootstrapTime => {
                let (server_time, delta) = MassaTime::from_bytes_compact(&buffer[cursor..])?;
                cursor += delta;

                let (version, delta) = Version::from_bytes_compact(&buffer[cursor..])?;
                cursor += delta;

                let config_hash = Hash::from_bytes(&array_from_slice(&buffer[cursor..])?)?;
                cursor += HASH_SIZE_BYTES;
                BootstrapMessage::BootstrapTime {
                    server_time,
                    version,
                    config_hash,
                }
            }
            MessageTypeId::Peers => {
                let (peers, delta) = BootstrapPeers::from_bytes_compact(&buffer[cursor..])?;
                cursor += delta;

                BootstrapMessage::BootstrapPeers { peers }
            }
            MessageTypeId::ConsensusState => {
                let (pos, delta) = ExportProofOfStake::from_bytes_compact(&buffer[cursor..])?;
                cursor += delta;
                let (graph, delta) = BootstrapableGraph::from_bytes_compact(&buffer[cursor..])?;
                cursor += delta;

                BootstrapMessage::ConsensusState { pos, graph }
            }
            MessageTypeId::ExecutionState => {
                let (execution_state, delta) =
                    BootstrapExecutionState::from_bytes_compact(&buffer[cursor..])?;
                cursor += delta;

                BootstrapMessage::ExecutionState { execution_state }
            }
        };
        Ok((res, cursor))
    }
}
