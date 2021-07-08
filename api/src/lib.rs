mod config;
mod error;
mod filters;

#[cfg(test)]
mod tests;

use communication::{network::NetworkConfig, protocol::ProtocolConfig};
pub use config::ApiConfig;
use config::CHANNEL_SIZE;
use consensus::ConsensusConfig;
use filters::get_filter;
use logging::massa_trace;
use models::SerializationContext;
use std::collections::VecDeque;
use storage::StorageAccess;
use tokio::sync::mpsc;

pub use error::ApiError;
pub use filters::{ApiEvent, ApiManagementCommand};

pub struct ApiEventReceiver(mpsc::Receiver<ApiEvent>);

pub struct ApiManager {
    join_handle: tokio::task::JoinHandle<()>,
    manager_tx: mpsc::Sender<ApiManagementCommand>,
}

/// Spawn API server.
///
pub async fn start_api_controller(
    cfg: ApiConfig,
    consensus_config: ConsensusConfig,
    protocol_config: ProtocolConfig,
    network_config: NetworkConfig,
    opt_storage_command_sender: Option<StorageAccess>,
    clock_compensation: i64,
    context: SerializationContext,
) -> Result<(ApiEventReceiver, ApiManager), ApiError> {
    let (event_tx, event_rx) = mpsc::channel::<ApiEvent>(CHANNEL_SIZE);
    let (manager_tx, mut manager_rx) = mpsc::channel::<ApiManagementCommand>(1);
    massa_trace!("api.lib.start_api_controller", {});
    let bind = cfg.bind;
    let (_addr, server) = warp::serve(get_filter(
        cfg,
        consensus_config,
        protocol_config,
        network_config,
        event_tx,
        opt_storage_command_sender,
        clock_compensation,
        context,
    ))
    .try_bind_with_graceful_shutdown(bind, async move {
        loop {
            massa_trace!("api.lib.start_api_controller.select", {});
            tokio::select! {
                cmd = manager_rx.recv() => {
                    massa_trace!("api.lib.start_api_controller.manager", {});
                    match cmd {
                        None => break,
                        Some(_) => {}
                    }
                }
            }
        }
    })?;

    let join_handle = tokio::task::spawn(server);

    Ok((
        ApiEventReceiver(event_rx),
        ApiManager {
            join_handle,
            manager_tx,
        },
    ))
}

impl ApiEventReceiver {
    /// Listen for ApiEvents
    pub async fn wait_event(&mut self) -> Result<ApiEvent, ApiError> {
        self.0.recv().await.ok_or(ApiError::SendChannelError(
            "could not receive api event".to_string(),
        ))
    }

    /// drains remaining events and returns them in a VecDeque
    /// note: events are sorted from oldest to newest
    pub async fn drain(mut self) -> VecDeque<ApiEvent> {
        let mut remaining_events: VecDeque<ApiEvent> = VecDeque::new();
        while let Some(evt) = self.0.recv().await {
            remaining_events.push_back(evt);
        }
        remaining_events
    }
}

impl ApiManager {
    /// Stop the protocol controller
    pub async fn stop(
        self,
        api_event_receiver: ApiEventReceiver,
    ) -> Result<VecDeque<ApiEvent>, ApiError> {
        massa_trace!("api.lib.stop", {});
        drop(self.manager_tx);
        let remaining_events = api_event_receiver.drain().await;
        let _ = self.join_handle.await?;
        Ok(remaining_events)
    }
}
