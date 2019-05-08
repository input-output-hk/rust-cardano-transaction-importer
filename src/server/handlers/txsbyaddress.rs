use iron::request::Request;
use iron::response::Response;
use iron::status;
use iron::IronResult;
use router::Router;

use crate::storage::transactions_of;
use crate::Config;

use cardano::address::ExtendedAddr;
use std::str::FromStr;
use std::sync::Arc;

pub struct Handler {
    config: Arc<Config>,
}

impl Handler {
    pub fn new(config: Arc<Config>) -> Self {
        Handler { config }
    }

    pub fn route(self, router: &mut Router) -> &mut Router {
        router.get("/transactions/:address", self, "transactionsbyaddress")
    }
}

impl iron::Handler for Handler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let params = req.extensions.get::<router::Router>().unwrap();
        let address_str = params.find("address").unwrap();

        let address = match ExtendedAddr::from_str(&address_str) {
            Ok(addr) => addr,
            Err(_) => return Ok(Response::with((status::BadRequest, "Invalid address"))),
        };

        let conn = match self.config.pool.get() {
            Ok(c) => c,
            Err(_) => {
                panic!("Couldn't get a connection to the database");
            }
        };

        let transactions = transactions_of(&conn, address).unwrap();

        let serialized = serde_json::to_string(&transactions).unwrap();

        let mut response = Response::with((status::Ok, serialized));
        response.headers.set(iron::headers::ContentType::json());

        Ok(response)
    }
}
