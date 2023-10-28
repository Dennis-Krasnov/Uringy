// run with `cargo test --test '*' --features http`

#[test]
fn http() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/http/*.rs");
}
