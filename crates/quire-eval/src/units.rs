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
    Some(ctx)
}

/// The unit engine for one evaluation pass. `None` results (a
/// prelude that failed to load) simply leave the scalar engine
/// alone: sheets evaluate exactly as before, minus the unit path.
pub struct Bridge {
    ctx: Context,
    units: HashSet<String>,
}

impl Bridge {
    pub fn new() -> Option<Self> {
        MASTER.with(|master| {
            let ctx = master.as_ref()?.clone();
            let units: HashSet<String> = ctx
                .unit_names()
                .iter()
                .flatten()
                .map(|n| n.to_string())
                .collect();
            Some(Bridge { ctx, units })
        })
    }

    /// Whether this identifier acts as a unit in the engine. Registry
    /// names (`g`, `meter`) are in the preloaded set; prefixed forms
    /// (`kg`, `mm`) are not names at all but parse-time prefix+unit
    /// combinations, so the engine's own parse of a bare `1 name`
    /// probe is the only truth. The probe is an expression statement:
    /// success binds nothing, failure rolls the typechecker back.
    pub fn knows_unit(&mut self, name: &str) -> bool {
        self.units.contains(name)
            || self
                .ctx
                .interpret(&format!("1 {name}"), CodeSource::Internal)
                .is_ok()
    }

    /// Interpret one compiled line, returning its value. Errors come
    /// back as their first message line, ready for an error cell.
    pub fn eval(&mut self, source: &str) -> Result<Value, String> {
        match self.ctx.interpret(source, CodeSource::Internal) {
            Ok((_, numbat::InterpreterResult::Value(v))) => Ok(v),
            Ok((_, numbat::InterpreterResult::Continue)) => {
                Err("the line produced no value".into())
            }
            Err(e) => Err(first_line(&e.to_string())),
        }
    }
}

/// The displayed form of a unit value, which is also its parseable
/// source (`5.3 kg` reads back as `5.3 kg`).
pub fn render(v: &Value) -> String {
    v.to_string()
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
    fn scalars_seed_as_plain_numbers() {
        let mut b = bridge();
        let v = b.eval("let milk = 3.5\nmilk * 2 kg").expect("evaluates");
        assert_eq!(render(&v), "7 kg");
    }
}
