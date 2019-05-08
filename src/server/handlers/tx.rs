use iron::request::Request;
use iron::response::Response;
use iron::status;
use iron::IronResult;
use router::Router;

use crate::storage::transaction;
use crate::Config;

use std::sync::Arc;

pub struct Handler {
    config: Arc<Config>,
}

impl Handler {
    pub fn new(config: Arc<Config>) -> Self {
        Handler { config }
    }

    pub fn route(self, router: &mut Router) -> &mut Router {
        router.get("/transaction/:tx", self, "transaction")
    }
}

impl iron::Handler for Handler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let params = req.extensions.get::<router::Router>().unwrap();
        let txid_str = params.find("tx").unwrap();

        let conn = match self.config.pool.get() {
            Ok(c) => c,
            Err(_) => {
                panic!("Couldn't get a connection to the database");
            }
        };

        let transaction = transaction(&conn, txid_str.to_string()).unwrap();

        let serialized = serde_json::to_string(&transaction).unwrap();

        let mut response = Response::with((status::Ok, serialized));
        response.headers.set(iron::headers::ContentType::json());

        Ok(response)
    }
}
