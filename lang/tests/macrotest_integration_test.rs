/// These tests use [macrotest](https://docs.rs/macrotest) to track changes to code generation
/// behavior.
///
/// If this test is failing, it means you have added code that altered the generated code for one of
/// the test files. If you _meant_ to change codegen behavior, simply run the `regen_expanded.sh`
/// script to update the .expanded.rs files and then sanity check that the updates match what you
/// intended.

#[test]
fn expand_generics_test() {
    macrotest::expand("tests/expand/generics_test.rs");
}

#[test]
fn expand_counter_test() {
    macrotest::expand("tests/expand/counter_test.rs");
}
