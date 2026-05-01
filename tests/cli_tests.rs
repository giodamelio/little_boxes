#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .case("tests/cmd/*.md")
        .case("tests/cmd/stdin.toml")
        .case("README.md");
}
