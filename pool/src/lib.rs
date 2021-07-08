// Copyright (c) 2021 MASSA LABS <info@massa.net>

#![feature(map_first_last)]

#[macro_use]
extern crate logging;

mod config;
mod error;
mod operation_pool;
mod pool_controller;
mod pool_worker;

pub use config::PoolConfig;
pub use error::PoolError;
pub use pool_controller::{start_pool_controller, PoolCommandSender, PoolManager};
pub use pool_worker::PoolCommand;

#[cfg(test)]
mod tests;
