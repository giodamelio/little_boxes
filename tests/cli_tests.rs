#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .default_bin_name("little_boxes")
        .case("tests/cmd/**/*.md")
        .case("tests/cmd/**/*.toml")
        .case("README.md");
}
