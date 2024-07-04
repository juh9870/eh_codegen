use diagnostic::context::DiagnosticContext;
use diagnostic::diagnostic::DiagnosticKind;
use owo_colors::{AnsiColors, OwoColorize};

pub fn report_diagnostics(ctx: DiagnosticContext) {
    for (entry, diagnostics) in ctx.diagnostics {
        let is_builtin = entry.starts_with("auto-") || entry.starts_with("eh-");
        let filtered: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                match d.kind {
                    DiagnosticKind::ObsoleteField => {
                        if is_builtin {
                            return false;
                        }
                        if d.path.last_is_field("cell_type")
                            || d.path.last_is_field("weapon_slot_type")
                            || d.path.last_is_field("not_available_in_game")
                        {
                            return false;
                        }
                    }
                    DiagnosticKind::ValueTooSmall { value, .. } => {
                        if d.path.last_is_field("barrel_id") && value == -1.0 {
                            return false;
                        }
                        if is_builtin && d.path.to_string().ends_with("<HaveQuestItem>.min_value") {
                            return false;
                        }
                    }
                    DiagnosticKind::ValueTooLarge { .. } => {}
                    DiagnosticKind::LayoutNotSquare { .. } => {}
                }
                true
            })
            .collect();

        if filtered.is_empty() {
            continue;
        }

        println!("\n{} {}:", "Diagnostics for".bright_black(), entry.bold());
        for diagnostic in filtered {
            let color = if diagnostic.kind.is_error() {
                AnsiColors::Red
            } else {
                AnsiColors::Yellow
            };
            println!(
                "{}: {}",
                diagnostic.path.bold(),
                diagnostic.kind.color(color)
            );
        }
    }
}
