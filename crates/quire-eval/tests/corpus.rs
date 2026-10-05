//! Adversarial corpus: the engine evaluates anything without
//! panicking. Every expression line yields an outcome; every other
//! line yields none. Errors are values, never crashes.

use quire_eval::{LineKind, evaluate_sheet};

#[test]
fn nasty_sheets_never_panic() {
    let mut cases: Vec<String> = [
        "",
        "\n\n\n",
        "   ",
        "// only a comment",
        "2 +",
        "+ 2",
        "* 2",
        "2 * * 3",
        "((((",
        "))))",
        "()",
        "(())",
        "1.2.3",
        ".....",
        "2 3 4",
        "= = =",
        "total total",
        "of of of",
        "2 of 3",
        "total * total",
        "answer answer",
        "%%%%%",
        "1e999",
        "milk milk milk",
        "milk = = 3",
        "x = (y = 2)",
        "5 %%% 2",
        "0 / 0",
        "2^10000",
        "\u{0662} + \u{0662}",
        "\u{1F680} * 3",
        "\u{200F}total\u{200F}",
        "x = 2^(2^(2^(2^(2^2))))",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    // a caret chain under the depth guard
    cases.push("2".to_string() + &"^2".repeat(30));
    // unary chains past the depth guard
    cases.push("-".repeat(5000) + "5");
    // parentheses past the depth guard
    cases.push(format!("{}1{}", "(".repeat(5000), ")".repeat(5000)));
    // a very long literal: refused at lex time, not panicked on
    cases.push("9".repeat(10_000));

    for (i, sheet) in cases.iter().enumerate() {
        for line in evaluate_sheet(sheet) {
            if line.kind == LineKind::Expression {
                assert!(
                    line.outcome.is_some(),
                    "case {i}: expression line {} produced no outcome",
                    line.number
                );
            } else {
                assert!(
                    line.outcome.is_none(),
                    "case {i}: non-expression line {} produced an outcome",
                    line.number
                );
            }
        }
    }
}
