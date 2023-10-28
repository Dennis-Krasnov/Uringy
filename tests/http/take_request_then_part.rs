use uringy::ecosystem::http::server::from_request::Query;
use uringy::ecosystem::http::server::routing::{get, Router};
use uringy::ecosystem::http::Request;

fn main() {
    fn root(_: Request, _: Query<()>) {}
    Router::new().route("/", get(root));
}
