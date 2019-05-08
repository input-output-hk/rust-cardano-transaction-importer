extern crate cardano;
extern crate exe_common;
extern crate r2d2;
extern crate r2d2_sqlite;
extern crate reqwest;
extern crate rusqlite;

extern crate iron;
extern crate router;

extern crate serde;
#[macro_use]
extern crate log;
extern crate env_logger;

mod config;
mod http_bridge;
mod server;
mod storage;
mod types;

use storage::{apply_initial_state, prepare_schema};

use cardano::block::types::HeaderHash;
use std::sync::Arc;
use std::{thread, time};

type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
type Config = config::Config<http_bridge::HttpBridge>;
use clap::{App, SubCommand};

use cardano::block::types::EpochId;

use crate::http_bridge::HttpBridgeApi;

fn main() -> rusqlite::Result<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let matches = App::new(clap::crate_name!())
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .subcommand(SubCommand::with_name("start").about("start server"))
        .subcommand(SubCommand::with_name("sync-block-index"))
        .get_matches();

    let mut settings = ::config::Config::default();
    settings
        .merge(::config::File::with_name("Settings.toml"))
        .unwrap();

    debug!("Settings :: {:?}", &settings);

    let port: u16 = settings.get("port").unwrap();
    let bridge_url = format!(
        "{}{}/",
        settings.get::<String>("http-bridge").unwrap(),
        settings.get::<String>("network").unwrap()
    );
    let refresh_interval = settings.get("refresh-interval").unwrap();
    let database: String = settings.get("database").unwrap();

    let manager = r2d2_sqlite::SqliteConnectionManager::file(database);
    let bridge = http_bridge::HttpBridge::new(bridge_url);
    let config = Arc::new(Config::new(port, bridge, manager, refresh_interval));

    match matches.subcommand() {
        ("start", Some(_)) => {
            let _server = crate::server::start_http_server(config.clone());
            loop {
                let config = config.clone();
                info!("Starting sync thread");
                let handle = thread::spawn(move || sync(config));

                if let Err(e) = handle.join().unwrap() {
                    error!("Syncing error: {}", e);
                }

                let restart_time = time::Duration::from_secs(5);
                info!("Syncing thread restarting in {} seconds", restart_time.as_secs());
                thread::sleep(restart_time);
            }
        }
        ("sync-block-index", Some(_)) => {
            let mut conn = config.pool.get().unwrap();

            match prepare_schema(&conn) {
                Err(e) => error!("Error preparing schema {}", e),
                _ => info!("Schema prepared"),
            }

            let genesis_hash = &config.genesis_prev;

            let chain_state =
                cardano::block::ChainState::new(&exe_common::genesisdata::parse::parse(
                    exe_common::genesisdata::data::get_genesis_data(&genesis_hash)
                        .unwrap()
                        .as_bytes(),
                ));

            match apply_initial_state(&mut conn, &chain_state.utxos) {
                Err(e) => error!("Could not apply initial state {}", e),
                _ => info!("Initial state applied"),
            }

            let first_unstable_epoch = config
                .bridge
                .first_unstable_epoch(config.epoch_stability_depth)
                .unwrap();

            info!("First unstable epoch: {}", first_unstable_epoch);

            storage::sync_from_epochs(&mut conn, first_unstable_epoch, |id: EpochId| {
                config.bridge.get_epoch(id).unwrap()
            })
            .unwrap();
        }
        _ => error!("Unrecognized argument"),
    };

    Ok(())
}

use std::result::Result;
fn sync(config: Arc<Config>) -> Result<(), types::Error> {
    let mut conn = config.pool.get().unwrap();

    loop {
        let tip = config.bridge.get_tip()?;

        info!("Tip is {}", tip.compute_hash());

        storage::update_block_index(&mut conn, tip.compute_hash(), |blockid: HeaderHash| {
            Ok(config
                .bridge
                .get_block(&blockid)?
                .header()
                .previous_header())
        })?;

        info!("Block index updated");

        let mut block_hash = storage::last_applied_block(&conn)?.unwrap();

        let transaction = conn.transaction().unwrap();

        let mut counter = 0;
        while let Some(next) = storage::next_block(&transaction, block_hash)? {
            counter = counter + 1;
            let block = config.bridge.get_block(&next)?;
            storage::apply_block(&transaction, &block)?;
            block_hash = next;
        }

        transaction.commit()?;

        info!("{} blocks applied", counter);
        info!("new head: {}", storage::last_applied_block(&conn)?.unwrap());

        thread::sleep(time::Duration::from_millis(config.refresh_interval));
    }
}
