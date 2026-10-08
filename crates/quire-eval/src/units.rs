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

use std::cell::RefCell;
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
    /// knows_unit probe answers, memoized per process. The master
    /// never mutates, so a probe's outcome can never change; probes
    /// cost a full master clone + parse each, and prose lines probe
    /// every word they contain on every pass.
    static PROBE_MEMO: RefCell<std::collections::HashMap<String, bool>> =
        RefCell::new(std::collections::HashMap::new());
}

/// The engine's own answer to `does `1 {name}` parse as a quantity`,
/// cached. First probe per name clones the master and tries; every
/// later probe is a set lookup.
fn probe_memo(name: &str) -> bool {
    PROBE_MEMO.with(|memo| {
        // exact name: numbat units are case-sensitive (K is kelvin,
        // k is kilo)
        if let Some(hit) = memo.borrow().get(name) {
            return *hit;
        }
        let answer = MASTER.with(|master| {
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
        });
        memo.borrow_mut().insert(name.to_string(), answer);
        answer
    })
}

fn build_master() -> Option<Context> {
    let mut ctx = Context::new(BuiltinModuleImporter::default());
    if ctx.interpret("use prelude", CodeSource::Internal).is_err() {
        return None;
    }
    // the recurrence periods' quarter: the prelude carries week,
    // month, and year but no quarter, and `N/quarter` routes here on
    // the strength of this registration (spec.md "Recurring amounts")
    let _ = ctx.interpret(
        "@aliases(quarters)\nunit quarter: Time = 3 months",
        CodeSource::Internal,
    );
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
    fns: HashSet<String>,
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
            let fns: HashSet<String> = master
                .as_ref()?
                .function_names()
                .map(|n| n.to_string())
                .collect();
            Some(Bridge { units, fns })
        })
    }

    /// Whether this identifier acts as a unit in the engine. Registry
    /// names (`g`, `meter`) are in the preloaded set; prefixed forms
    /// (`kg`) are not names at all but parse-time prefix+unit
    /// combinations, so the engine's own parse of a bare `1 name`
    /// probe is the only truth. The probe is an expression statement:
    /// success binds nothing, failure rolls the typechecker back.
    pub fn knows_unit(&self, name: &str) -> bool {
        self.units.contains(name) || probe_memo(name)
    }

    /// Whether this identifier is a prelude function the bridge can
    /// evaluate (`sqrt`, `log10`, `sin`, `abs`, and friends): the
    /// routing key that lets `sqrt(144)` leave the scalar path.
    pub fn knows_function(&self, name: &str) -> bool {
        self.fns.contains(name)
    }

    /// Interpret one compiled line, returning its value. Errors come
    /// back as their first message line, ready for an error cell.
    pub fn eval(&self, source: &str) -> Result<Value, String> {
        MASTER.with(|master| {
            let mut ctx = master.clone().unwrap();
            match ctx.interpret(source, CodeSource::Internal) {
                Ok((_, numbat::InterpreterResult::Value(v))) => Ok(v),
                Ok((_, numbat::InterpreterResult::Continue)) => {
                    Err("the line produced no value".into())
                }
                Err(e) => Err(first_line(&e.to_string())),
            }
        })
    }

    /// The engine's completable names for a prefix: the roadmap trio
    /// (numbat's own completion gatherer, the unit registry, and the
    /// variable namespace) merged and prefix-filtered. Two documented
    /// gaps: prefixed symbols (`kg`) are parse-time combinations, not
    /// registry names, and typechecker constants (`pi`) have no
    /// public accessor - both evaluate fine when typed (see
    /// `knows_unit`). Currency codes are not candidates either: the
    /// currencies module defines each unit's value from the exchange
    /// rates AT LOAD TIME (NaN when rates are absent), so it must
    /// only ever load on demand, into a context that is about to
    /// interpret - never into the long-lived master completions read.
    pub fn completions(&self, prefix: &str) -> Vec<String> {
        MASTER.with(|master| {
            let Some(master) = master.as_ref() else {
                return Vec::new();
            };
            // three sources (the roadmap's trio): numbat's own
            // completion gatherer (units and their long forms), the
            // unit registry flattened (short aliases like `pi` and
            // `hours`), and the variable namespace (runtime names;
            // thin on the prelude-only master)
            let mut names: Vec<String> = master.get_completions_for(prefix, false).collect();
            names.extend(
                master
                    .unit_names()
                    .iter()
                    .flatten()
                    .filter(|n| n.starts_with(prefix))
                    .map(|n| n.to_string()),
            );
            names.extend(
                master
                    .variable_names()
                    .filter(|n| n.starts_with(prefix))
                    .map(|n| n.to_string()),
            );
            names
        })
    }
}

/// The displayed form of a unit value, which is also its parseable
/// source (`5.3 kg` reads back as `5.3 kg`). Datetimes render
/// human-readable instead: `2026-10-28 00:00 +02:00`. A bare rate (a
/// value per period, the recurrence phrases' shape) reads
/// `1200 /month` rather than the engine's `1200 month⁻¹`; the slash
/// form is what the sheet wrote, and numbat parses it back the same.
pub fn render(v: &Value) -> String {
    match v {
        Value::DateTime(dt) => dt.strftime("%Y-%m-%d %H:%M %Z").to_string(),
        other => polish_rate(&other.to_string()),
    }
}

/// Rewrite the engine's numerator-less rate display (`1200
/// month⁻¹`) into the sheet's slash form (`1200 /month`). Compound
/// units, money rates (which the engine already prints with a
/// slash), and anything whose unit stem is not a plain name pass
/// through untouched; the result must stay parseable source, since
/// totals and `&N` refs re-feed rendered values to the engine.
fn polish_rate(text: &str) -> String {
    let Some((value, unit)) = text.rsplit_once(' ') else {
        return text.to_string();
    };
    let Some(stem) = unit.strip_suffix("⁻¹") else {
        return text.to_string();
    };
    if stem.is_empty() || !stem.chars().all(|c| c.is_ascii_alphanumeric()) {
        return text.to_string();
    }
    format!("{value} /{stem}")
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

/// Completion candidates for the word before the cursor (the Tab
/// completion, spec.md "Completion"): sheet names bound above the
/// cursor line plus the engine's own names, filtered to the prefix.
pub fn completion_names(text: &str, cursor_line: usize, prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = crate::index_sheet(text)
        .assignments
        .into_iter()
        .filter(|(line, _)| *line < cursor_line)
        .map(|(_, name)| name)
        .filter(|name| name.starts_with(prefix))
        .collect();
    if let Some(bridge) = Bridge::new() {
        names.extend(bridge.completions(prefix));
    }
    names.retain(|name| name.starts_with(prefix) && name != prefix);
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bridge() -> Bridge {
        Bridge::new().expect("the prelude must load from the built-in modules")
    }

    #[test]
    fn prelude_loads_without_importer_or_network() {
        let b = bridge();
        assert!(b.knows_unit("kg"));
        assert!(b.knows_unit("m"));
        assert!(!b.knows_unit("bloognorch"));
    }

    #[test]
    fn quantities_evaluate_and_render_as_source() {
        let b = bridge();
        let v = b.eval("5 kg + 300 g").expect("evaluates");
        assert_eq!(render(&v), "5300 g");
    }

    #[test]
    fn dimension_mismatch_is_an_error_message() {
        let b = bridge();
        let err = b.eval("3 kg + 5 m").expect_err("must fail");
        assert!(err.contains("Mass") && err.contains("Length"), "{err}");
    }

    #[test]
    fn currency_converts_once_rates_exist() {
        use_test_rates();
        let b = bridge();
        let v = b.eval("50 USD -> EUR").expect("converts with rates");
        assert_eq!(render(&v), "50 \u{20ac}");
    }

    #[test]
    fn scalars_seed_as_plain_numbers() {
        let b = bridge();
        let v = b.eval("let milk = 3.5\nmilk * 2 kg").expect("evaluates");
        assert_eq!(render(&v), "7 kg");
    }

    #[test]
    fn quarter_registers_beside_the_prelude() {
        let b = bridge();
        assert!(b.knows_unit("quarter"));
        let v = b.eval("1 quarter -> days").expect("evaluates");
        assert_eq!(render(&v), "91.3105 day");
    }

    #[test]
    fn bare_rates_render_in_slash_form() {
        let b = bridge();
        let v = b.eval("1200/month").expect("evaluates");
        assert_eq!(render(&v), "1200 /month");
        let v = b.eval("5000/year").expect("evaluates");
        // the engine's year prints as its short alias
        assert_eq!(render(&v), "5000 /yr");
        // money rates keep the engine's own slash form, untouched
        use_test_rates();
        let b = bridge();
        let v = b.eval("1200 USD / month").expect("evaluates");
        assert_eq!(render(&v), "1200 $/month");
        // plain quantities pass the polish by
        let v = b.eval("5 kg + 300 g").expect("evaluates");
        assert_eq!(render(&v), "5300 g");
    }

    #[test]
    fn rendered_rates_read_back_as_source() {
        // totals and &N refs re-feed rendered values to the engine,
        // so the polished display must parse back to the same value
        let b = bridge();
        let text = render(&b.eval("1200/month").expect("evaluates"));
        let v = b.eval(&format!("({text}) -> 1/day")).expect("reparses");
        assert_eq!(render(&v), "39.4259 /day");
    }

    #[test]
    fn completions_reach_the_engine_and_the_sheet() {
        // engine names: registry units and their aliases (an exact
        // match is not its own completion)
        let names = completion_names("", 1, "hour");
        assert!(names.contains(&"hours".to_string()), "{names:?}");
        // sheet names bound above the cursor line only, prefix-filtered
        let text = "rate = 12\nrotor = 2\n";
        let names = completion_names(text, 2, "r");
        assert!(names.contains(&"rate".to_string()), "{names:?}");
        assert!(!names.contains(&"rotor".to_string()), "{names:?}");
        // an exact match is not its own completion
        let names = completion_names(text, 3, "rate");
        assert!(!names.contains(&"rate".to_string()));
    }
}
