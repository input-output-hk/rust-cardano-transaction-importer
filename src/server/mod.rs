use router::Router;
use iron::Iron;
use iron::Listening;

mod handlers;

use handlers::txsbyaddress;
use handlers::tx;
use crate::Config;
use log::info;
use std::sync::Arc;

pub fn start_http_server(config: Arc<Config>) -> Listening {
    let mut router = Router::new();

    txsbyaddress::Handler::new(config.clone()).route(&mut router);
    tx::Handler::new(config.clone()).route(&mut router);
    
    info!("listening to port {}", config.port);
    Iron::new(router).http(format!("0.0.0.0:{}", config.port))
        .expect("start http server")
}