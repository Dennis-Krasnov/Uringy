use http::StatusCode;
use uringy::ecosystem::http::server::routing::{get, Router};

fn main() {
    Router::new().route("/", get(|| ("hello", StatusCode::OK)));
}
