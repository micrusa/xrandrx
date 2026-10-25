use crate::models::{Alignment, CliOptions};

/// Parses command line arguments into `CliOptions`.
pub fn parse_args<I, T>(args: I) -> Result<CliOptions, String>
where
    I: IntoIterator<Item = T>,
    T: Into<String>,
{
    let mut opts = CliOptions::default();
    let raw_args: Vec<String> = args.into_iter().map(|s| s.into()).collect();
    let mut iter = raw_args.into_iter().peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--version" | "-v" => {
                println!("xrandrx {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--list" | "-l" | "list" => {
                opts.list = true;
            }
            "--best" => {
                opts.best = true;
            }
            "--no-best" => {
                opts.best = false;
            }
            "--ltr" => {
                opts.alignment = Alignment::LeftToRight;
                opts.explicit_layout = true;
            }
            "--rtl" => {
                opts.alignment = Alignment::RightToLeft;
                opts.explicit_layout = true;
            }
            "--mirror" | "-m" => {
                opts.mirror = true;
                opts.explicit_layout = true;
            }
            "--dry-run" | "-n" => {
                opts.dry_run = true;
            }
            "--keep-others" | "--no-off" => {
                opts.off_unused = false;
            }
            "--primary" | "-p" => {
                let next_val = iter.next().ok_or_else(|| {
                    "Option '--primary' requires an argument (monitor name/alias)".to_string()
                })?;
                opts.primary = Some(next_val);
            }
            s if s.starts_with("--primary=") => {
                let val = s["--primary=".len()..].to_string();
                if val.is_empty() {
                    return Err("Option '--primary' requires a monitor name".to_string());
                }
                opts.primary = Some(val);
            }
            s if s.starts_with('-') => {
                return Err(format!("Unknown option '{}'. Use --help for usage.", s));
            }
            screen => {
                opts.screens.push(screen.to_string());
                opts.explicit_layout = true;
            }
        }
    }

    Ok(opts)
}

pub fn print_help() {
    print!(r#"xrandrx - Effortless multi-monitor layout manager for xrandr

USAGE:
    xrandrx [OPTIONS] [SCREENS...]

EXAMPLES:
    xrandrx --ltr hdmi edp dp
    xrandrx --rtl dp edp hdmi
    xrandrx --mirror hdmi edp
    xrandrx --ltr hdmi edp dp --primary edp
    xrandrx --dry-run --ltr hdmi edp dp
    xrandrx --list

OPTIONS:
    --ltr                Align screens Left to Right (default)
    --rtl                Align screens Right to Left
    --mirror, -m         Mirror screens (clone mode)
    --best               Select maximum resolution & refresh rate (default: on)
    --primary <SCREEN>   Specify primary display (default: center screen)
    --dry-run, -n        Print xrandr command without applying changes
    --list, -l           List connected monitors and their available modes
    --keep-others        Do not turn off unselected connected displays
    -h, --help           Show help information
    -v, --version        Show version
"#);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_defaults() {
        let args = vec!["--ltr", "hdmi", "edp", "dp"];
        let opts = parse_args(args).unwrap();
        assert_eq!(opts.alignment, Alignment::LeftToRight);
        assert!(!opts.mirror);
        assert_eq!(opts.screens, vec!["hdmi", "edp", "dp"]);
        assert!(opts.best);
        assert!(opts.primary.is_none());
        assert!(!opts.dry_run);
        assert!(!opts.list);
        assert!(opts.explicit_layout);
    }

    #[test]
    fn test_parse_args_rtl_and_primary() {
        let args = vec!["--rtl", "hdmi", "edp", "--primary", "edp", "--dry-run"];
        let opts = parse_args(args).unwrap();
        assert_eq!(opts.alignment, Alignment::RightToLeft);
        assert!(!opts.mirror);
        assert_eq!(opts.screens, vec!["hdmi", "edp"]);
        assert_eq!(opts.primary.as_deref(), Some("edp"));
        assert!(opts.dry_run);
        assert!(opts.explicit_layout);
    }

    #[test]
    fn test_parse_args_mirror() {
        let args = vec!["--mirror", "hdmi", "edp"];
        let opts = parse_args(args).unwrap();
        assert!(opts.mirror);
        assert_eq!(opts.screens, vec!["hdmi", "edp"]);
        assert!(opts.explicit_layout);
    }

    #[test]
    fn test_parse_args_primary_equals() {
        let args = vec!["--primary=hdmi0", "hdmi0", "edp"];
        let opts = parse_args(args).unwrap();
        assert_eq!(opts.primary.as_deref(), Some("hdmi0"));
        assert_eq!(opts.screens, vec!["hdmi0", "edp"]);
        assert!(opts.explicit_layout);
    }
}
