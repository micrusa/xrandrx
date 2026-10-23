use crate::models::Output;

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedSelector {
    pub family: String,
    pub index: Option<usize>,
}

/// Parses strings like "hdmi", "hdmi0", "hdmi1", "edp", "dp2" into family + index.
pub fn parse_selector(input: &str) -> Option<ParsedSelector> {
    let lower = input.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }

    // Check known connector families (order matters: edp before dp)
    let known_families = ["edp", "hdmi", "dp", "vga", "dvi", "virtual"];
    for &fam in &known_families {
        if lower.starts_with(fam) {
            let rest = &lower[fam.len()..];
            let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
            let index = if digits.is_empty() {
                None
            } else {
                digits.parse::<usize>().ok()
            };
            return Some(ParsedSelector {
                family: fam.to_string(),
                index,
            });
        }
    }

    // Generic fallback: alphabetical prefix + digits
    let alpha_part: String = lower.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    if !alpha_part.is_empty() {
        let rest = &lower[alpha_part.len()..];
        let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
        let index = if digits.is_empty() {
            None
        } else {
            digits.parse::<usize>().ok()
        };
        return Some(ParsedSelector {
            family: alpha_part,
            index,
        });
    }

    None
}

/// Identifies the connector family of an xrandr output name (e.g. "HDMI-0-1" -> "hdmi", "eDP-1" -> "edp").
pub fn get_output_family(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    if lower.starts_with("edp") {
        "edp".to_string()
    } else if lower.starts_with("hdmi") {
        "hdmi".to_string()
    } else if lower.starts_with("dp") {
        "dp".to_string()
    } else if lower.starts_with("vga") {
        "vga".to_string()
    } else if lower.starts_with("dvi") {
        "dvi".to_string()
    } else if lower.starts_with("virtual") {
        "virtual".to_string()
    } else {
        let prefix: String = lower.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        if prefix.is_empty() {
            lower
        } else {
            prefix
        }
    }
}

/// Resolves a user-friendly monitor name or alias to a connected `Output`.
pub fn match_output<'a>(
    pattern: &str,
    connected_outputs: &'a [Output],
) -> Result<&'a Output, String> {
    if connected_outputs.is_empty() {
        return Err("No connected outputs detected from xrandr".to_string());
    }

    let pattern_trimmed = pattern.trim();

    // 1. Exact case-insensitive match on actual device name (e.g. "HDMI-1", "eDP-1-1"):
    if let Some(out) = connected_outputs
        .iter()
        .find(|o| o.name.eq_ignore_ascii_case(pattern_trimmed))
    {
        return Ok(out);
    }

    // 2. Family + index enumeration match (e.g. "hdmi", "hdmi0", "hdmi1", "edp", "dp1"):
    if let Some(selector) = parse_selector(pattern_trimmed) {
        let mut family_matches: Vec<&'a Output> = connected_outputs
            .iter()
            .filter(|o| get_output_family(&o.name) == selector.family)
            .collect();

        // Sort naturally so HDMI-1 is before HDMI-2, HDMI-0-1 before HDMI-0-2
        family_matches.sort_by(|a, b| a.name.cmp(&b.name));

        if !family_matches.is_empty() {
            match selector.index {
                None => {
                    // Single match or default to first match:
                    // "if there's a single hdmi entry called HDMI-0-1 or whatever, that can be used with hdmi.
                    // Otherwise they can be enumerated like hdmi0 hdmi1, hdmi will be the first one"
                    return Ok(family_matches[0]);
                }
                Some(idx) => {
                    if idx < family_matches.len() {
                        return Ok(family_matches[idx]);
                    } else {
                        return Err(format!(
                            "Selector '{}' requested index {} for family '{}', but only {} connected output(s) found ({})",
                            pattern_trimmed,
                            idx,
                            selector.family,
                            family_matches.len(),
                            family_matches
                                .iter()
                                .map(|o| o.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
            }
        }
    }

    // 3. Exact alphanumeric match (ignoring hyphens, underscores, dots, etc.)
    let norm_pattern: String = pattern_trimmed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    let norm_matches: Vec<&'a Output> = connected_outputs
        .iter()
        .filter(|o| {
            let norm_name: String = o
                .name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            norm_name == norm_pattern
        })
        .collect();

    if norm_matches.len() == 1 {
        return Ok(norm_matches[0]);
    }

    // 4. Substring fallback match:
    let sub_matches: Vec<&'a Output> = connected_outputs
        .iter()
        .filter(|o| o.name.to_ascii_lowercase().contains(&norm_pattern))
        .collect();

    if sub_matches.len() == 1 {
        return Ok(sub_matches[0]);
    }

    let available: Vec<&str> = connected_outputs.iter().map(|o| o.name.as_str()).collect();
    Err(format!(
        "Could not match monitor selector '{}'. Available connected monitors: {}",
        pattern_trimmed,
        available.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Mode;

    fn make_test_output(name: &str) -> Output {
        Output {
            name: name.to_string(),
            connected: true,
            is_primary: false,
            current_mode: None,
            current_pos: None,
            modes: vec![Mode {
                width: 1920,
                height: 1080,
                refresh_rate: 60.0,
                is_preferred: true,
                is_current: false,
            }],
        }
    }

    #[test]
    fn test_single_hdmi_resolution() {
        let outputs = vec![
            make_test_output("HDMI-0-1"),
            make_test_output("eDP-1"),
            make_test_output("DP-1"),
        ];

        let matched = match_output("hdmi", &outputs).unwrap();
        assert_eq!(matched.name, "HDMI-0-1");

        let matched_0 = match_output("hdmi0", &outputs).unwrap();
        assert_eq!(matched_0.name, "HDMI-0-1");

        let err = match_output("hdmi1", &outputs);
        assert!(err.is_err());
    }

    #[test]
    fn test_multiple_hdmi_resolution() {
        let outputs = vec![
            make_test_output("HDMI-1"),
            make_test_output("HDMI-2"),
            make_test_output("eDP-1"),
        ];

        // "hdmi" defaults to the first one
        let matched = match_output("hdmi", &outputs).unwrap();
        assert_eq!(matched.name, "HDMI-1");

        // "hdmi0" is first one
        let matched0 = match_output("hdmi0", &outputs).unwrap();
        assert_eq!(matched0.name, "HDMI-1");

        // "hdmi1" is second one
        let matched1 = match_output("hdmi1", &outputs).unwrap();
        assert_eq!(matched1.name, "HDMI-2");
    }

    #[test]
    fn test_edp_and_dp_distinction() {
        let outputs = vec![
            make_test_output("eDP-1"),
            make_test_output("DP-1"),
            make_test_output("DP-2"),
        ];

        let edp = match_output("edp", &outputs).unwrap();
        assert_eq!(edp.name, "eDP-1");

        let dp = match_output("dp", &outputs).unwrap();
        assert_eq!(dp.name, "DP-1");

        let dp0 = match_output("dp0", &outputs).unwrap();
        assert_eq!(dp0.name, "DP-1");

        let dp1 = match_output("dp1", &outputs).unwrap();
        assert_eq!(dp1.name, "DP-2");
    }

    #[test]
    fn test_exact_match() {
        let outputs = vec![
            make_test_output("HDMI-A-1"),
            make_test_output("DP-1-3"),
        ];

        let matched = match_output("HDMI-A-1", &outputs).unwrap();
        assert_eq!(matched.name, "HDMI-A-1");

        let matched_lower = match_output("hdmi-a-1", &outputs).unwrap();
        assert_eq!(matched_lower.name, "HDMI-A-1");
    }
}
