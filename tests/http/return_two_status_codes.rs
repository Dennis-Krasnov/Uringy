use http::StatusCode;
use uringy::ecosystem::http::server::routing::{get, Router};

fn main() {
    // this is valid in Axum
    Router::new().route("/", get(|| (StatusCode::OK, StatusCode::OK)));
}
