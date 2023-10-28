use uringy::ecosystem::http::server::routing::{get, Router};
use uringy::ecosystem::http::Request;

fn main() {
    fn root(_: Request, _: Request) {}
    Router::new().route("/", get(root));
}
