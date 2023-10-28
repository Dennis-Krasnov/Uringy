use http::{HeaderMap, StatusCode};
use uringy::ecosystem::http::server::routing::{get, Router};

fn main() {
    Router::new().route("/", get(|| (HeaderMap::new(), StatusCode::OK)));
}
