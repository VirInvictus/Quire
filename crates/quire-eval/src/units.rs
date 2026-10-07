//! The unit bridge: an embedded numbat context that evaluates the
//! expression lines the scalar engine declines (spec.md "Unit
//! expressions"). The prelude loads from numbat's built-in modules,
//! so there is no filesystem and no network: the crate is built with
//! its fetch and plotting features off.
//!
//! The bridge owns one `Context` per sheet-evaluation pass. Quire
//! drives it with compiled source: seed `let`s for the sheet
//! bindings a line references, then the line itself (variable
//! statements become numbat `let`s, since `=` in numbat is
//! comparison).

use std::collections::HashSet;

use numbat::Context;
use numbat::module_importer::BuiltinModuleImporter;
use numbat::resolver::CodeSource;
use numbat::value::Value;

// The prelude costs 30-170 ms to interpret (release/debug) — far too
// much for a per-keystroke evaluation pass. One master context loads
// it once per process; each pass clones it (~0.2-0.8 ms) and mutates
// the clone, so the master stays pristine.
thread_local! {
    static MASTER: Option<Context> = build_master();
}

fn build_master() -> Option<Context> {
    let mut ctx = Context::new(BuiltinModuleImporter::default());
    if ctx.interpret("use prelude", CodeSource::Internal).is_err() {
        return None;
    }
    // the currency module loads lazily, only when a sheet names a
    // currency AND rates exist (spec.md "Unit expressions": the app
    // seeds rates from its ECB cache before currency is used)
    ctx.load_currency_module_on_demand(true);
    Some(ctx)
}

thread_local! {
    static RATES_SEEDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Seed the engine's exchange-rate cache from an ECB XML snapshot
/// (the app's `~/.cache/quire/ecb.xml`). Set-once per process -
/// numbat's rates cache cannot be replaced after the first read, so
/// a refresh applies from the next launch (spec.md "Currency").
/// Thread-safe: the fetch thread calls this as freely as startup.
pub fn set_exchange_rates(xml: &str) {
    RATES_SEEDED.with(|seeded| {
        if !seeded.get() {
            Context::set_exchange_rates(xml);
            seeded.set(true);
        }
    });
}

/// Install numbat's test rates (every currency at 1.0). Test-only.
pub fn use_test_rates() {
    Context::use_test_exchange_rates();
}

/// The unit engine for one evaluation pass. `None` results (a
/// prelude that failed to load) simply leave the scalar engine
/// alone: sheets evaluate exactly as before, minus the unit path.
///
/// The bridge holds no context of its own: every evaluation clones
/// the pristine master, so seeds (variables, function definitions)
/// from one line never leak into the next and mid-sheet
/// redefinitions apply (numbat forbids re-defining in a shared
/// context; a fresh clone has nothing to clash with).
pub struct Bridge {
    units: HashSet<String>,
}

impl Bridge {
    pub fn new() -> Option<Self> {
        MASTER.with(|master| {
            let units: HashSet<String> = master
                .as_ref()?
                .unit_names()
                .iter()
                .flatten()
                .map(|n| n.to_string())
                .collect();
            Some(Bridge { units })
        })
    }

    /// Whether this identifier acts as a unit in the engine. Registry
    /// names (`g`, `meter`) are in the preloaded set; prefixed forms
    /// (`kg`, `mm`) are not names at all but parse-time prefix+unit
    /// combinations, so the engine's own parse of a bare `1 name`
    /// probe is the only truth. The probe is an expression statement:
    /// success binds nothing, failure rolls the typechecker back.
    pub fn knows_unit(&self, name: &str) -> bool {
        self.units.contains(name)
            || MASTER.with(|master| {
                master
                    .as_ref()
                    .and_then(|m| {
                        let mut probe = m.clone();
                        probe
                            .interpret(&format!("1 {name}"), CodeSource::Internal)
                            .ok()
                            .map(|_| ())
                    })
                    .is_some()
            })
    }

    /// Interpret one compiled line, returning its value. Errors come
    /// back as their first message line, ready for an error cell.
    pub fn eval(&self, source: &str) -> Result<Value, String> {
        MASTER.with(|master| {
            let Some(mut ctx) = master.as_ref().map(|m| m.clone()) else {
                return Err("the unit engine is unavailable".into());
            };
            match ctx.interpret(source, CodeSource::Internal) {
                Ok((_, numbat::InterpreterResult::Value(v))) => Ok(v),
                Ok((_, numbat::InterpreterResult::Continue)) => {
                    Err("the line produced no value".into())
                }
                Err(e) => Err(first_line(&e.to_string())),
            }
        })
    }
}

/// The displayed form of a unit value, which is also its parseable
/// source (`5.3 kg` reads back as `5.3 kg`). Datetimes render
/// human-readable instead: `2026-10-28 00:00 +02:00`.
pub fn render(v: &Value) -> String {
    match v {
        Value::DateTime(dt) => dt.strftime("%Y-%m-%d %H:%M %Z").to_string(),
        other => other.to_string(),
    }
}

/// A scalar binding seeded into the unit engine as a plain number.
pub fn scalar_source(v: f64) -> String {
    format!("{v}")
}

fn first_line(message: &str) -> String {
    // numbat's error Display is short multi-line detail ("left hand
    // side: Mass" / "right hand side: Length"); an error cell wants
    // one compact line, so the details collapse onto one.
    let compact = message
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    if compact.is_empty() {
        "unit error".into()
    } else {
        compact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bridge() -> Bridge {
        Bridge::new().expect("the prelude must load from the built-in modules")
    }

    #[test]
    fn prelude_loads_without_importer_or_network() {
        let mut b = bridge();
        assert!(b.knows_unit("kg"));
        assert!(b.knows_unit("m"));
        assert!(!b.knows_unit("bloognorch"));
    }

    #[test]
    fn quantities_evaluate_and_render_as_source() {
        let mut b = bridge();
        let v = b.eval("5 kg + 300 g").expect("evaluates");
        assert_eq!(render(&v), "5300 g");
    }

    #[test]
    fn dimension_mismatch_is_an_error_message() {
        let mut b = bridge();
        let err = b.eval("3 kg + 5 m").expect_err("must fail");
        assert!(err.contains("Mass") && err.contains("Length"), "{err}");
    }

    #[test]
    fn currency_converts_once_rates_exist() {
        use_test_rates();
        let mut b = bridge();
        let v = b.eval("50 USD -> EUR").expect("converts with rates");
        assert_eq!(render(&v), "50 \u{20ac}");
    }

    #[test]
    fn scalars_seed_as_plain_numbers() {
        let mut b = bridge();
        let v = b.eval("let milk = 3.5\nmilk * 2 kg").expect("evaluates");
        assert_eq!(render(&v), "7 kg");
    }
}
