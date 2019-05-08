use crate::http_bridge::HttpBridgeApi;
use crate::Pool;
use cardano::block::types::HeaderHash;
use r2d2_sqlite::SqliteConnectionManager;

#[derive(Clone)]
pub struct Config<T: HttpBridgeApi> {
    pub genesis_prev: HeaderHash,
    pub genesis: HeaderHash,
    pub pool: Pool,
    pub port: u16,
    pub bridge: T,
    pub epoch_stability_depth: usize,
    pub refresh_interval: u64,
}

impl<T: HttpBridgeApi> Config<T> {
    pub fn new(
        port: u16,
        bridge: T,
        connection_manager: SqliteConnectionManager,
        refresh_interval: u64,
    ) -> Self {
        let cfg = exe_common::config::net::Config::mainnet();
        let pool = r2d2::Pool::new(connection_manager).unwrap();

        Config {
            genesis_prev: cfg.genesis_prev,
            pool,
            port,
            bridge,
            refresh_interval,
            genesis: cfg.genesis,
            epoch_stability_depth: cfg.epoch_stability_depth,
        }
    }
}
