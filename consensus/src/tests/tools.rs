// Copyright (c) 2021 MASSA LABS <info@massa.net>

use super::mock_pool_controller::{MockPoolController, PoolCommandSink};
use super::mock_protocol_controller::MockProtocolController;
use crate::{
    block_graph::{BlockGraphExport, ExportActiveBlock},
    ledger::LedgerData,
    pos::{RollCounts, RollUpdate, RollUpdates},
    ConsensusConfig,
};
use crate::{
    start_consensus_controller, BootsrapableGraph, ConsensusCommandSender, ConsensusEventReceiver,
    ExportProofOfStake,
};
use communication::protocol::ProtocolCommand;
use crypto::{
    hash::Hash,
    signature::{PrivateKey, PublicKey},
};
use models::{
    Address, Amount, Block, BlockHeader, BlockHeaderContent, BlockId, Operation, OperationContent,
    OperationType, SerializeCompact, Slot,
};
use pool::PoolCommand;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    path::Path,
};
use storage::{StorageAccess, StorageConfig};
use tempfile::NamedTempFile;
use time::UTime;

pub fn get_dummy_block_id(s: &str) -> BlockId {
    BlockId(Hash::hash(s.as_bytes()))
}

//return true if another block has been seen
pub async fn validate_notpropagate_block(
    protocol_controller: &mut MockProtocolController,
    not_propagated: BlockId,
    timeout_ms: u64,
) -> bool {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::IntegratedBlock { block_id, .. } => return Some(block_id),
            _ => None,
        })
        .await;
    match param {
        Some(block_id) => !(not_propagated == block_id),
        None => false,
    }
}

//return true if another block has been seen
pub async fn validate_notpropagate_block_in_list(
    protocol_controller: &mut MockProtocolController,
    not_propagated: &Vec<BlockId>,
    timeout_ms: u64,
) -> bool {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::IntegratedBlock { block_id, .. } => return Some(block_id),
            _ => None,
        })
        .await;
    match param {
        Some(block_id) => !not_propagated.contains(&block_id),
        None => false,
    }
}

pub async fn validate_propagate_block_in_list(
    protocol_controller: &mut MockProtocolController,
    valid: &Vec<BlockId>,
    timeout_ms: u64,
) -> BlockId {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::IntegratedBlock { block_id, .. } => return Some(block_id),
            _ => None,
        })
        .await;
    match param {
        Some(block_id) => {
            assert!(valid.contains(&block_id), "not the valid hash propagated");
            block_id
        }
        None => panic!("Hash not propagated."),
    }
}

pub async fn validate_ask_for_block(
    protocol_controller: &mut MockProtocolController,
    valid: BlockId,
    timeout_ms: u64,
) -> BlockId {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::WishlistDelta { new, .. } => return Some(new),
            _ => None,
        })
        .await;
    match param {
        Some(new) => {
            assert!(new.contains(&valid), "not the valid hash asked for");
            assert_eq!(new.len(), 1);
            valid
        }
        None => panic!("Block not asked for before timeout."),
    }
}

pub async fn validate_wishlist(
    protocol_controller: &mut MockProtocolController,
    new: HashSet<BlockId>,
    remove: HashSet<BlockId>,
    timeout_ms: u64,
) {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::WishlistDelta { new, remove } => return Some((new, remove)),
            _ => None,
        })
        .await;
    match param {
        Some((got_new, got_remove)) => {
            assert_eq!(new, got_new);
            assert_eq!(remove, got_remove);
        }
        None => panic!("Wishlist delta not sent for before timeout."),
    }
}

pub async fn validate_does_not_ask_for_block(
    protocol_controller: &mut MockProtocolController,
    hash: &BlockId,
    timeout_ms: u64,
) {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::WishlistDelta { new, .. } => return Some(new),
            _ => None,
        })
        .await;
    match param {
        Some(new) => {
            if new.contains(hash) {
                panic!("unexpected ask for block {:?}", hash);
            }
        }
        None => {}
    }
}

pub async fn validate_propagate_block(
    protocol_controller: &mut MockProtocolController,
    valid_hash: BlockId,
    timeout_ms: u64,
) {
    protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::IntegratedBlock { block_id, .. } => {
                if block_id == valid_hash {
                    return Some(());
                }
                None
            }
            _ => None,
        })
        .await
        .expect("Block not propagated before timeout.")
}

pub async fn validate_notify_block_attack_attempt(
    protocol_controller: &mut MockProtocolController,
    valid_hash: BlockId,
    timeout_ms: u64,
) {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::AttackBlockDetected(hash) => return Some(hash),
            _ => None,
        })
        .await;
    match param {
        Some(hash) => assert_eq!(valid_hash, hash, "Attack attempt notified for wrong hash."),
        None => panic!("Attack attempt not notified before timeout."),
    }
}

pub fn start_storage() -> StorageAccess {
    let tempdir = tempfile::tempdir().expect("cannot create temp dir");
    let storage_config = StorageConfig {
        /// Max number of bytes we want to store
        max_stored_blocks: 50,
        /// path to db
        path: tempdir.path().to_path_buf(), //in target to be ignored by git and different file between test.
        cache_capacity: 256,  //little to force flush cache
        flush_interval: None, //defaut
        reset_at_startup: true,
    };
    let (storage_command_tx, _storage_manager) = storage::start_storage(storage_config).unwrap();
    storage_command_tx
}

pub async fn validate_block_found(
    protocol_controller: &mut MockProtocolController,
    valid_hash: &BlockId,
    timeout_ms: u64,
) {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::GetBlocksResults(results) => return Some(results),
            _ => None,
        })
        .await;
    match param {
        Some(results) => {
            let found = results
                .get(valid_hash)
                .expect("Hash not found in results")
                .is_some();
            assert!(
                found,
                "Get blocks results does not contain the expected results."
            );
        }
        None => panic!("Get blocks results not sent before timeout."),
    }
}

pub async fn validate_block_not_found(
    protocol_controller: &mut MockProtocolController,
    valid_hash: &BlockId,
    timeout_ms: u64,
) {
    let param = protocol_controller
        .wait_command(timeout_ms.into(), |cmd| match cmd {
            ProtocolCommand::GetBlocksResults(results) => return Some(results),
            _ => None,
        })
        .await;
    match param {
        Some(results) => {
            let not_found = results
                .get(valid_hash)
                .expect("Hash not found in results")
                .is_none();
            assert!(
                not_found,
                "Get blocks results does not contain the expected results."
            );
        }
        None => panic!("Get blocks results not sent before timeout."),
    }
}

pub async fn create_and_test_block(
    protocol_controller: &mut MockProtocolController,
    cfg: &ConsensusConfig,
    slot: Slot,
    best_parents: Vec<BlockId>,
    valid: bool,
    trace: bool,
    creator: PrivateKey,
) -> BlockId {
    let (block_hash, block, _) = create_block(&cfg, slot, best_parents, creator);
    if trace {
        info!("create block:{}", block_hash);
    }

    protocol_controller.receive_block(block).await;
    if valid {
        // Assert that the block is propagated.
        validate_propagate_block(protocol_controller, block_hash, 2000).await;
    } else {
        // Assert that the the block is not propagated.
        validate_notpropagate_block(protocol_controller, block_hash, 500).await;
    }
    block_hash
}

pub async fn propagate_block(
    protocol_controller: &mut MockProtocolController,
    block: Block,
    valid: bool,
    timeout_ms: u64,
) -> BlockId {
    let block_hash = block.header.compute_block_id().unwrap();
    protocol_controller.receive_block(block).await;
    if valid {
        //see if the block is propagated.
        validate_propagate_block(protocol_controller, block_hash, timeout_ms).await;
    } else {
        //see if the block is propagated.
        validate_notpropagate_block(protocol_controller, block_hash, timeout_ms).await;
    }
    block_hash
}

pub fn create_roll_transaction(
    priv_key: PrivateKey,
    sender_public_key: PublicKey,
    roll_count: u64,
    buy: bool,
    expire_period: u64,
    fee: u64,
) -> Operation {
    let op = if buy {
        OperationType::RollBuy { roll_count }
    } else {
        OperationType::RollSell { roll_count }
    };

    let content = OperationContent {
        sender_public_key,
        fee: Amount::from(fee),
        expire_period,
        op,
    };
    let hash = Hash::hash(&content.to_bytes_compact().unwrap());
    let signature = crypto::sign(&hash, &priv_key).unwrap();
    Operation { content, signature }
}

pub async fn wait_pool_slot(
    pool_controller: &mut MockPoolController,
    t0: UTime,
    period: u64,
    thread: u8,
) -> Slot {
    pool_controller
        .wait_command(t0.checked_mul(2).unwrap(), |cmd| match cmd {
            PoolCommand::UpdateCurrentSlot(s) => {
                if s >= Slot::new(period, thread) {
                    Some(s)
                } else {
                    None
                }
            }
            _ => None,
        })
        .await
        .expect("timeout while waiting for slot")
}

pub fn create_transaction(
    priv_key: PrivateKey,
    sender_public_key: PublicKey,
    recipient_address: Address,
    amount: u64,
    expire_period: u64,
    fee: u64,
) -> Operation {
    let op = OperationType::Transaction {
        recipient_address,
        amount: Amount::from(amount),
    };

    let content = OperationContent {
        sender_public_key,
        fee: Amount::from(fee),
        expire_period,
        op,
    };
    let hash = Hash::hash(&content.to_bytes_compact().unwrap());
    let signature = crypto::sign(&hash, &priv_key).unwrap();
    Operation { content, signature }
}

pub fn create_roll_buy(
    priv_key: PrivateKey,
    roll_count: u64,
    expire_period: u64,
    fee: u64,
) -> Operation {
    let op = OperationType::RollBuy { roll_count };
    let sender_public_key = crypto::derive_public_key(&priv_key);
    let content = OperationContent {
        sender_public_key,
        fee: Amount::from(fee),
        expire_period,
        op,
    };
    let hash = Hash::hash(&content.to_bytes_compact().unwrap());
    let signature = crypto::sign(&hash, &priv_key).unwrap();
    Operation { content, signature }
}

pub fn create_roll_sell(
    priv_key: PrivateKey,
    roll_count: u64,
    expire_period: u64,
    fee: u64,
) -> Operation {
    let op = OperationType::RollSell { roll_count };
    let sender_public_key = crypto::derive_public_key(&priv_key);
    let content = OperationContent {
        sender_public_key,
        fee: Amount::from(fee),
        expire_period,
        op,
    };
    let hash = Hash::hash(&content.to_bytes_compact().unwrap());
    let signature = crypto::sign(&hash, &priv_key).unwrap();
    Operation { content, signature }
}

// returns hash and resulting discarded blocks
pub fn create_block(
    cfg: &ConsensusConfig,
    slot: Slot,
    best_parents: Vec<BlockId>,
    creator: PrivateKey,
) -> (BlockId, Block, PrivateKey) {
    create_block_with_merkle_root(
        cfg,
        Hash::hash("default_val".as_bytes()),
        slot,
        best_parents,
        creator,
    )
}

// returns hash and resulting discarded blocks
pub fn create_block_with_merkle_root(
    _cfg: &ConsensusConfig,
    operation_merkle_root: Hash,
    slot: Slot,
    best_parents: Vec<BlockId>,
    creator: PrivateKey,
) -> (BlockId, Block, PrivateKey) {
    let public_key = crypto::derive_public_key(&creator);
    let (hash, header) = BlockHeader::new_signed(
        &creator,
        BlockHeaderContent {
            creator: public_key,
            slot,
            parents: best_parents,
            operation_merkle_root,
        },
    )
    .unwrap();

    let block = Block {
        header,
        operations: Vec::new(),
    };

    (hash, block, creator)
}

pub fn get_export_active_test_block(
    creator: PublicKey,
    parents: Vec<(BlockId, u64)>,
    operations: Vec<Operation>,
    slot: Slot,
    is_final: bool,
) -> (ExportActiveBlock, BlockId) {
    let block = Block {
        header: BlockHeader {
            content: BlockHeaderContent{
                creator: creator,
                operation_merkle_root: Hash::hash(&operations.iter().map(|op|{
                    op
                        .get_operation_id()
                        .unwrap()
                        .to_bytes()
                        .clone()
                    })
                    .flatten()
                    .collect::<Vec<_>>()[..]),
                parents: parents.iter()
                    .map(|(id,_)| *id)
                    .collect(),
                slot,
            },
            signature: crypto::signature::Signature::from_bs58_check(
                "5f4E3opXPWc3A1gvRVV7DJufvabDfaLkT1GMterpJXqRZ5B7bxPe5LoNzGDQp9LkphQuChBN1R5yEvVJqanbjx7mgLEae"
            ).unwrap()
        },
        operations: operations.clone(),
    };
    let id = block.header.compute_block_id().unwrap();
    (
        ExportActiveBlock {
            parents,
            dependencies: vec![],
            block,
            children: vec![vec![], vec![]],
            is_final,
            block_ledger_changes: vec![],
            roll_updates: vec![],
        },
        id,
    )
}

pub fn create_block_with_operations(
    _cfg: &ConsensusConfig,
    slot: Slot,
    best_parents: &Vec<BlockId>,
    creator: PrivateKey,
    operations: Vec<Operation>,
) -> (BlockId, Block, PrivateKey) {
    let public_key = crypto::derive_public_key(&creator);

    let operation_merkle_root = Hash::hash(
        &operations.iter().fold(Vec::new(), |acc, v| {
            let res = [acc, v.to_bytes_compact().unwrap()].concat();
            res
        })[..],
    );

    let (hash, header) = BlockHeader::new_signed(
        &creator,
        BlockHeaderContent {
            creator: public_key,
            slot,
            parents: best_parents.clone(),
            operation_merkle_root,
        },
    )
    .unwrap();

    let block = Block { header, operations };

    (hash, block, creator)
}

/// generate a named temporary JSON ledger file
pub fn generate_ledger_file(ledger_vec: &HashMap<Address, LedgerData>) -> NamedTempFile {
    use std::io::prelude::*;
    let ledger_file_named = NamedTempFile::new().expect("cannot create temp file");
    serde_json::to_writer_pretty(ledger_file_named.as_file(), &ledger_vec)
        .expect("unable to write ledger file");
    ledger_file_named
        .as_file()
        .seek(std::io::SeekFrom::Start(0))
        .expect("could not seek file");
    ledger_file_named
}

pub fn generate_staking_keys_file(staking_keys: &Vec<PrivateKey>) -> NamedTempFile {
    use std::io::prelude::*;
    let file_named = NamedTempFile::new().expect("cannot create temp file");
    serde_json::to_writer_pretty(file_named.as_file(), &staking_keys)
        .expect("unable to write ledger file");
    file_named
        .as_file()
        .seek(std::io::SeekFrom::Start(0))
        .expect("could not seek file");
    file_named
}

/// generate a named temporary JSON initial rolls file
pub fn generate_roll_counts_file(roll_counts: &RollCounts) -> NamedTempFile {
    use std::io::prelude::*;
    let roll_counts_file_named = NamedTempFile::new().expect("cannot create temp file");
    serde_json::to_writer_pretty(roll_counts_file_named.as_file(), &roll_counts.0)
        .expect("unable to write ledger file");
    roll_counts_file_named
        .as_file()
        .seek(std::io::SeekFrom::Start(0))
        .expect("could not seek file");
    roll_counts_file_named
}

/// generate a default named temporary JSON initial rolls file,
/// asuming two threads.
pub fn generate_default_roll_counts_file(stakers: Vec<PrivateKey>) -> NamedTempFile {
    let mut roll_counts = RollCounts::default();
    for key in stakers.iter() {
        let pub_key = crypto::derive_public_key(key);
        let address = Address::from_public_key(&pub_key).unwrap();
        let update = RollUpdate {
            roll_purchases: 1,
            roll_sales: 0,
        };
        let mut updates = RollUpdates::default();
        updates.apply(&address, &update).unwrap();
        roll_counts.apply_updates(&updates).unwrap();
    }
    generate_roll_counts_file(&roll_counts)
}

pub fn get_creator_for_draw(draw: &Address, nodes: &Vec<PrivateKey>) -> PrivateKey {
    for key in nodes.iter() {
        let pub_key = crypto::derive_public_key(key);
        let address = Address::from_public_key(&pub_key).unwrap();
        if address == *draw {
            return key.clone();
        }
    }
    panic!("Matching key for draw not found.");
}

pub fn default_consensus_config(
    initial_ledger_path: &Path,
    roll_counts_path: &Path,
    staking_keys_path: &Path,
) -> ConsensusConfig {
    let genesis_key = crypto::generate_random_private_key();
    let thread_count: u8 = 2;
    let max_block_size: u32 = 3 * 1024 * 1024;
    let max_operations_per_block: u32 = 1024;
    let tempdir = tempfile::tempdir().expect("cannot create temp dir");

    // Init the serialization context with a default,
    // can be overwritten with a more specific one in the test.
    models::init_serialization_context(models::SerializationContext {
        max_block_operations: 1024,
        parent_count: 2,
        max_peer_list_length: 128,
        max_message_size: 3 * 1024 * 1024,
        max_block_size: 3 * 1024 * 1024,
        max_bootstrap_blocks: 100,
        max_bootstrap_cliques: 100,
        max_bootstrap_deps: 100,
        max_bootstrap_children: 100,
        max_ask_blocks_per_message: 10,
        max_operations_per_message: 1024,
        max_bootstrap_message_size: 100000000,
        max_bootstrap_pos_entries: 1000,
        max_bootstrap_pos_cycles: 5,
    });

    ConsensusConfig {
        genesis_timestamp: UTime::now(0).unwrap(),
        thread_count: thread_count,
        t0: 32000.into(),
        genesis_key,
        max_discarded_blocks: 10,
        future_block_processing_max_periods: 3,
        max_future_processing_blocks: 10,
        max_dependency_blocks: 10,
        delta_f0: 32,
        disable_block_creation: true,
        max_block_size,
        max_operations_per_block,
        operation_validity_periods: 1,
        ledger_path: tempdir.path().to_path_buf(),
        ledger_cache_capacity: 1000000,
        ledger_flush_interval: Some(200.into()),
        ledger_reset_at_startup: true,
        block_reward: 1,
        initial_ledger_path: initial_ledger_path.to_path_buf(),
        operation_batch_size: 100,
        initial_rolls_path: roll_counts_path.to_path_buf(),
        initial_draw_seed: "genesis".into(),
        periods_per_cycle: 100,
        pos_lookback_cycles: 2,
        pos_lock_cycles: 1,
        pos_draw_cached_cycles: 0,
        roll_price: 0,
        stats_timespan: 60000.into(),
        staking_keys_path: staking_keys_path.to_path_buf(),
        end_timestamp: None,
    }
}

/// Runs a consensus test, passing a mock pool controller to it.
pub async fn consensus_pool_test<F, V>(
    cfg: ConsensusConfig,
    opt_storage_command_sender: Option<StorageAccess>,
    boot_pos: Option<ExportProofOfStake>,
    boot_graph: Option<BootsrapableGraph>,
    test: F,
) where
    F: FnOnce(
        MockPoolController,
        MockProtocolController,
        ConsensusCommandSender,
        ConsensusEventReceiver,
    ) -> V,
    V: Future<
        Output = (
            MockPoolController,
            MockProtocolController,
            ConsensusCommandSender,
            ConsensusEventReceiver,
        ),
    >,
{
    // mock protocol & pool
    let (protocol_controller, protocol_command_sender, protocol_event_receiver) =
        MockProtocolController::new();
    let (pool_controller, pool_command_sender) = MockPoolController::new();

    // launch consensus controller
    let (consensus_command_sender, consensus_event_receiver, consensus_manager) =
        start_consensus_controller(
            cfg.clone(),
            protocol_command_sender,
            protocol_event_receiver,
            pool_command_sender,
            opt_storage_command_sender,
            boot_pos,
            boot_graph,
            0,
        )
        .await
        .expect("could not start consensus controller");

    // Call test func.
    let (
        pool_controller,
        mut protocol_controller,
        _consensus_command_sender,
        consensus_event_receiver,
    ) = test(
        pool_controller,
        protocol_controller,
        consensus_command_sender,
        consensus_event_receiver,
    )
    .await;

    // stop controller while ignoring all commands
    let stop_fut = consensus_manager.stop(consensus_event_receiver);
    let pool_sink = PoolCommandSink::new(pool_controller).await;
    tokio::pin!(stop_fut);
    protocol_controller
        .ignore_commands_while(stop_fut)
        .await
        .unwrap();
    pool_sink.stop().await;
}

/// Runs a consensus test, without passing a mock pool controller to it.
pub async fn consensus_without_pool_test<F, V>(
    cfg: ConsensusConfig,
    opt_storage_command_sender: Option<StorageAccess>,
    test: F,
) where
    F: FnOnce(MockProtocolController, ConsensusCommandSender, ConsensusEventReceiver) -> V,
    V: Future<
        Output = (
            MockProtocolController,
            ConsensusCommandSender,
            ConsensusEventReceiver,
        ),
    >,
{
    // mock protocol & pool
    let (protocol_controller, protocol_command_sender, protocol_event_receiver) =
        MockProtocolController::new();
    let (pool_controller, pool_command_sender) = MockPoolController::new();
    let pool_sink = PoolCommandSink::new(pool_controller).await;

    // launch consensus controller
    let (consensus_command_sender, consensus_event_receiver, consensus_manager) =
        start_consensus_controller(
            cfg.clone(),
            protocol_command_sender,
            protocol_event_receiver,
            pool_command_sender,
            opt_storage_command_sender,
            None,
            None,
            0,
        )
        .await
        .expect("could not start consensus controller");

    // Call test func.
    let (mut protocol_controller, _consensus_command_sender, consensus_event_receiver) = test(
        protocol_controller,
        consensus_command_sender,
        consensus_event_receiver,
    )
    .await;

    // stop controller while ignoring all commands
    let stop_fut = consensus_manager.stop(consensus_event_receiver);
    tokio::pin!(stop_fut);
    protocol_controller
        .ignore_commands_while(stop_fut)
        .await
        .unwrap();
    pool_sink.stop().await;
}

pub fn get_cliques(graph: &BlockGraphExport, hash: BlockId) -> HashSet<usize> {
    let mut res = HashSet::new();
    for (i, clique) in graph.max_cliques.iter().enumerate() {
        if clique.contains(&hash) {
            res.insert(i);
        }
    }
    res
}
