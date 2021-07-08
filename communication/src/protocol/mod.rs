// Copyright (c) 2021 MASSA LABS <info@massa.net>

mod config;
mod protocol_controller;
mod protocol_worker;

pub use config::ProtocolConfig;
pub use protocol_controller::{
    start_protocol_controller, ProtocolCommandSender, ProtocolEventReceiver, ProtocolManager,
    ProtocolPoolEventReceiver,
};
pub use protocol_worker::{ProtocolCommand, ProtocolEvent, ProtocolPoolEvent};

#[cfg(test)]
pub mod tests;
