use http::HeaderMap;

use uringy::ecosystem::http::server::routing::{get, Router};

fn main() {
    Router::new().route("/", get(|| ("hello", HeaderMap::new())));
}
