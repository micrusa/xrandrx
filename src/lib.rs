pub mod cli;
pub mod executor;
pub mod matcher;
pub mod models;
pub mod parser;
pub mod planner;

use executor::XrandrRunner;
use models::CliOptions;
use parser::parse_xrandr_output;
use planner::create_plan;

/// Main application logic executed with a given `XrandrRunner`.
/// Returns the generated command string upon success.
pub fn run_app(runner: &dyn XrandrRunner, opts: CliOptions) -> Result<String, String> {
    let raw_query = runner.query()?;
    let outputs = parse_xrandr_output(&raw_query);

    if opts.list {
        let mut report = String::new();
        report.push_str("Detected displays:\n");
        let connected: Vec<&models::Output> = outputs.iter().filter(|o| o.connected).collect();
        if connected.is_empty() {
            report.push_str("  (none connected)\n");
        } else {
            for out in connected {
                let best = out
                    .best_mode()
                    .map(|m| format!("{}@{:.2}Hz", m.mode_str(), m.refresh_rate))
                    .unwrap_or_else(|| "none".to_string());
                let cur = out
                    .current_mode
                    .as_deref()
                    .unwrap_or("off");
                let primary_tag = if out.is_primary { " [primary]" } else { "" };
                report.push_str(&format!(
                    "  * {} (current: {}, best: {}){}\n",
                    out.name, cur, best, primary_tag
                ));
            }
        }

        let disconnected: Vec<&models::Output> = outputs.iter().filter(|o| !o.connected).collect();
        if !disconnected.is_empty() {
            report.push_str("Disconnected:\n  ");
            let disc_names: Vec<&str> = disconnected.iter().map(|o| o.name.as_str()).collect();
            report.push_str(&disc_names.join(", "));
            report.push('\n');
        }

        print!("{}", report);
        return Ok(report);
    }

    let plan = create_plan(
        &outputs,
        &opts.screens,
        opts.alignment,
        opts.mirror,
        opts.primary.as_deref(),
        opts.best,
        opts.off_unused,
    )?;

    let cmd_str = plan.to_command_string();

    if opts.dry_run {
        println!("[dry-run] {}", cmd_str);
    } else {
        println!("Applying: {}", cmd_str);
        runner.apply(&plan.to_xrandr_args())?;
    }

    Ok(cmd_str)
}
