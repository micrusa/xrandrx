use crate::matcher::match_output;
use crate::models::{Alignment, Mode, Output, Plan, PlannedOutput};

/// Selects the primary output among the ordered visual monitors.
/// If `explicit_primary` is provided, matches against that output name.
/// Otherwise:
/// - If odd number of screens (e.g. 1, 3, 5): chooses the exact center screen (e.g. index N/2).
/// - If even number of screens (e.g. 2, 4): chooses between the two centered screens
///   (indices N/2 - 1 and N/2) whichever has higher resolution (pixels, then refresh rate).
pub fn select_primary_index(
    outputs: &[(&Output, &Mode)],
    explicit_primary: Option<&str>,
) -> Result<usize, String> {
    if outputs.is_empty() {
        return Err("Cannot determine primary output for empty monitor list".to_string());
    }

    if let Some(query) = explicit_primary {
        let query_trimmed = query.trim();
        for (i, (out, _)) in outputs.iter().enumerate() {
            if out.name.eq_ignore_ascii_case(query_trimmed) {
                return Ok(i);
            }
        }
        // Also check if matcher can resolve the query to one of the active outputs
        let matched_candidates: Vec<Output> = outputs.iter().map(|(o, _)| (*o).clone()).collect();
        if let Ok(matched) = match_output(query_trimmed, &matched_candidates) {
            for (i, (out, _)) in outputs.iter().enumerate() {
                if out.name == matched.name {
                    return Ok(i);
                }
            }
        }

        let active_names: Vec<&str> = outputs.iter().map(|(o, _)| o.name.as_str()).collect();
        return Err(format!(
            "Specified primary screen '{}' is not among the active layout screens: {}",
            query,
            active_names.join(", ")
        ));
    }

    let n = outputs.len();
    if n % 2 != 0 {
        // Odd number: exact center (e.g. n=3 -> index 1)
        Ok(n / 2)
    } else {
        // Even number: compare the two centered screens (n/2 - 1 and n/2)
        let idx1 = (n / 2) - 1;
        let idx2 = n / 2;

        let mode1 = outputs[idx1].1;
        let mode2 = outputs[idx2].1;

        let res1 = mode1.resolution_pixels();
        let res2 = mode2.resolution_pixels();

        if res1 > res2 {
            Ok(idx1)
        } else if res2 > res1 {
            Ok(idx2)
        } else {
            // Equal resolution: compare refresh rates
            if mode2.refresh_rate > mode1.refresh_rate {
                Ok(idx2)
            } else {
                Ok(idx1)
            }
        }
    }
}

/// Creates an execution plan for xrandr based on detected outputs, layout parameters, and alignment/mirroring.
pub fn create_plan(
    all_outputs: &[Output],
    selected_selectors: &[String],
    alignment: Alignment,
    mirror: bool,
    explicit_primary: Option<&str>,
    best: bool,
    off_unused: bool,
) -> Result<Plan, String> {
    let connected_outputs: Vec<&Output> = all_outputs.iter().filter(|o| o.connected).collect();

    if connected_outputs.is_empty() {
        return Err("No connected displays found".to_string());
    }

    // Determine the list of chosen outputs
    let chosen_outputs: Vec<&Output> = if selected_selectors.is_empty() {
        let mut list = connected_outputs.clone();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    } else {
        let connected_cloned: Vec<Output> = connected_outputs.iter().map(|&o| o.clone()).collect();
        let mut chosen = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for sel in selected_selectors {
            let matched = match_output(sel, &connected_cloned)?;
            if seen_names.contains(&matched.name) {
                return Err(format!("Display '{}' was specified more than once", matched.name));
            }
            seen_names.insert(matched.name.clone());

            let original = connected_outputs
                .iter()
                .find(|o| o.name == matched.name)
                .expect("Output exists in connected list");
            chosen.push(*original);
        }
        chosen
    };

    if mirror {
        // In mirror mode:
        // Find best common resolution among all chosen displays if best is true
        let common_mode_resolution = if best {
            let mut common: Vec<(u32, u32)> = Vec::new();
            if let Some(first) = chosen_outputs.first() {
                for m in &first.modes {
                    let res = (m.width, m.height);
                    if !common.contains(&res) {
                        let supported_by_all = chosen_outputs.iter().all(|out| {
                            out.modes.iter().any(|m2| m2.width == res.0 && m2.height == res.1)
                        });
                        if supported_by_all {
                            common.push(res);
                        }
                    }
                }
            }
            // Sort by pixel count descending
            common.sort_by_key(|&(w, h)| std::cmp::Reverse(w as u64 * h as u64));
            common.first().copied()
        } else {
            None
        };

        let mut outputs_with_modes: Vec<(&Output, Mode)> = Vec::new();
        for out in &chosen_outputs {
            let mode = if let Some((w, h)) = common_mode_resolution {
                out.modes
                    .iter()
                    .filter(|m| m.width == w && m.height == h)
                    .max_by(|a, b| {
                        a.refresh_rate
                            .partial_cmp(&b.refresh_rate)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .cloned()
                    .unwrap_or_else(|| out.best_mode().cloned().unwrap())
            } else if best {
                out.best_mode()
                    .cloned()
                    .ok_or_else(|| format!("Display '{}' has no available modes", out.name))?
            } else {
                out.modes
                    .iter()
                    .find(|m| m.is_current)
                    .cloned()
                    .or_else(|| out.best_mode().cloned())
                    .ok_or_else(|| format!("Display '{}' has no available modes", out.name))?
            };
            outputs_with_modes.push((*out, mode));
        }

        // Determine primary display
        let refs_for_primary: Vec<(&Output, &Mode)> = outputs_with_modes
            .iter()
            .map(|(out, mode)| (*out, mode))
            .collect();
        let primary_idx = select_primary_index(&refs_for_primary, explicit_primary)?;
        let primary_name = outputs_with_modes[primary_idx].0.name.clone();

        let mut active_outputs = Vec::new();
        for (i, (out, mode)) in outputs_with_modes.into_iter().enumerate() {
            let is_primary = i == primary_idx;
            let same_as = if is_primary {
                None
            } else {
                Some(primary_name.clone())
            };

            active_outputs.push(PlannedOutput {
                name: out.name.clone(),
                mode,
                pos_x: 0,
                pos_y: 0,
                is_primary,
                same_as,
            });
        }

        let mut turn_off_outputs = Vec::new();
        if off_unused {
            let active_set: std::collections::HashSet<&str> =
                active_outputs.iter().map(|p| p.name.as_str()).collect();

            for out in &connected_outputs {
                if !active_set.contains(out.name.as_str()) {
                    turn_off_outputs.push(out.name.clone());
                }
            }
            turn_off_outputs.sort();
        }

        return Ok(Plan {
            active_outputs,
            turn_off_outputs,
        });
    }

    // Extend mode (LTR or RTL)
    let mut outputs_with_modes: Vec<(&Output, Mode)> = Vec::new();
    for out in &chosen_outputs {
        let mode = if best {
            out.best_mode()
                .cloned()
                .ok_or_else(|| format!("Display '{}' has no available modes", out.name))?
        } else {
            out.modes
                .iter()
                .find(|m| m.is_current)
                .cloned()
                .or_else(|| out.best_mode().cloned())
                .ok_or_else(|| format!("Display '{}' has no available modes", out.name))?
        };
        outputs_with_modes.push((*out, mode));
    }

    let mut visual_order: Vec<(&Output, Mode)> = outputs_with_modes;
    if alignment == Alignment::RightToLeft {
        visual_order.reverse();
    }

    let refs_for_primary: Vec<(&Output, &Mode)> = visual_order
        .iter()
        .map(|(out, mode)| (*out, mode))
        .collect();
    let primary_idx = select_primary_index(&refs_for_primary, explicit_primary)?;

    let mut current_x = 0;
    let mut active_outputs = Vec::new();

    for (i, (out, mode)) in visual_order.into_iter().enumerate() {
        let is_primary = i == primary_idx;
        active_outputs.push(PlannedOutput {
            name: out.name.clone(),
            mode: mode.clone(),
            pos_x: current_x,
            pos_y: 0,
            is_primary,
            same_as: None,
        });
        current_x += mode.width;
    }

    let mut turn_off_outputs = Vec::new();
    if off_unused {
        let active_set: std::collections::HashSet<&str> =
            active_outputs.iter().map(|p| p.name.as_str()).collect();

        for out in &connected_outputs {
            if !active_set.contains(out.name.as_str()) {
                turn_off_outputs.push(out.name.clone());
            }
        }
        turn_off_outputs.sort();
    }

    Ok(Plan {
        active_outputs,
        turn_off_outputs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_output_with_mode(name: &str, width: u32, height: u32, refresh: f64) -> Output {
        Output {
            name: name.to_string(),
            connected: true,
            is_primary: false,
            current_mode: None,
            current_pos: None,
            modes: vec![Mode {
                width,
                height,
                refresh_rate: refresh,
                is_preferred: true,
                is_current: false,
            }],
        }
    }

    #[test]
    fn test_odd_center_primary() {
        let o1 = make_output_with_mode("HDMI-1", 1920, 1080, 60.0);
        let o2 = make_output_with_mode("eDP-1", 1920, 1080, 60.0);
        let o3 = make_output_with_mode("DP-1", 1920, 1080, 60.0);

        let m1 = o1.best_mode().unwrap();
        let m2 = o2.best_mode().unwrap();
        let m3 = o3.best_mode().unwrap();

        let list = vec![(&o1, m1), (&o2, m2), (&o3, m3)];
        let primary_idx = select_primary_index(&list, None).unwrap();
        assert_eq!(primary_idx, 1);
        assert_eq!(list[primary_idx].0.name, "eDP-1");
    }

    #[test]
    fn test_even_center_primary_higher_resolution() {
        let o0 = make_output_with_mode("DP-0", 1920, 1080, 60.0);
        let o1 = make_output_with_mode("DP-1", 1920, 1080, 60.0);
        let o2 = make_output_with_mode("DP-2", 2560, 1440, 60.0);
        let o3 = make_output_with_mode("DP-3", 1920, 1080, 60.0);

        let m0 = o0.best_mode().unwrap();
        let m1 = o1.best_mode().unwrap();
        let m2 = o2.best_mode().unwrap();
        let m3 = o3.best_mode().unwrap();

        let list = vec![(&o0, m0), (&o1, m1), (&o2, m2), (&o3, m3)];
        let primary_idx = select_primary_index(&list, None).unwrap();
        assert_eq!(primary_idx, 2);
        assert_eq!(list[primary_idx].0.name, "DP-2");
    }

    #[test]
    fn test_even_center_primary_equal_resolution_tiebreak() {
        let o0 = make_output_with_mode("DP-0", 1920, 1080, 60.0);
        let o1 = make_output_with_mode("DP-1", 1920, 1080, 144.0);

        let m0 = o0.best_mode().unwrap();
        let m1 = o1.best_mode().unwrap();

        let list = vec![(&o0, m0), (&o1, m1)];
        let primary_idx = select_primary_index(&list, None).unwrap();
        assert_eq!(primary_idx, 1);
        assert_eq!(list[primary_idx].0.name, "DP-1");
    }

    #[test]
    fn test_plan_ltr_and_rtl() {
        let all = vec![
            make_output_with_mode("HDMI-1", 1920, 1080, 60.0),
            make_output_with_mode("eDP-1", 1920, 1080, 60.0),
            make_output_with_mode("DP-1", 2560, 1440, 60.0),
        ];

        let plan_ltr = create_plan(
            &all,
            &["hdmi".into(), "edp".into(), "dp".into()],
            Alignment::LeftToRight,
            false,
            None,
            true,
            true,
        ).unwrap();

        assert_eq!(plan_ltr.active_outputs.len(), 3);
        assert_eq!(plan_ltr.active_outputs[0].name, "HDMI-1");
        assert_eq!(plan_ltr.active_outputs[0].pos_x, 0);

        assert_eq!(plan_ltr.active_outputs[1].name, "eDP-1");
        assert_eq!(plan_ltr.active_outputs[1].pos_x, 1920);
        assert!(plan_ltr.active_outputs[1].is_primary);

        assert_eq!(plan_ltr.active_outputs[2].name, "DP-1");
        assert_eq!(plan_ltr.active_outputs[2].pos_x, 1920 + 1920);

        let plan_rtl = create_plan(
            &all,
            &["hdmi".into(), "edp".into(), "dp".into()],
            Alignment::RightToLeft,
            false,
            None,
            true,
            true,
        ).unwrap();

        assert_eq!(plan_rtl.active_outputs.len(), 3);
        assert_eq!(plan_rtl.active_outputs[0].name, "DP-1");
        assert_eq!(plan_rtl.active_outputs[0].pos_x, 0);

        assert_eq!(plan_rtl.active_outputs[1].name, "eDP-1");
        assert_eq!(plan_rtl.active_outputs[1].pos_x, 2560);
        assert!(plan_rtl.active_outputs[1].is_primary);

        assert_eq!(plan_rtl.active_outputs[2].name, "HDMI-1");
        assert_eq!(plan_rtl.active_outputs[2].pos_x, 2560 + 1920);
    }

    #[test]
    fn test_plan_mirror() {
        let all = vec![
            Output {
                name: "HDMI-1".to_string(),
                connected: true,
                is_primary: false,
                current_mode: None,
                current_pos: None,
                modes: vec![
                    Mode { width: 2560, height: 1440, refresh_rate: 60.0, is_preferred: true, is_current: false },
                    Mode { width: 1920, height: 1080, refresh_rate: 60.0, is_preferred: false, is_current: false },
                ],
            },
            Output {
                name: "eDP-1".to_string(),
                connected: true,
                is_primary: false,
                current_mode: None,
                current_pos: None,
                modes: vec![
                    Mode { width: 1920, height: 1080, refresh_rate: 144.0, is_preferred: true, is_current: false },
                    Mode { width: 1920, height: 1080, refresh_rate: 60.0, is_preferred: false, is_current: false },
                ],
            },
        ];

        let plan = create_plan(
            &all,
            &["hdmi".into(), "edp".into()],
            Alignment::LeftToRight,
            true, // mirror
            None,
            true,
            true,
        ).unwrap();

        assert_eq!(plan.active_outputs.len(), 2);
        // Best common resolution is 1920x1080!
        assert_eq!(plan.active_outputs[0].mode.width, 1920);
        assert_eq!(plan.active_outputs[0].mode.height, 1080);
        assert_eq!(plan.active_outputs[0].pos_x, 0);

        assert_eq!(plan.active_outputs[1].mode.width, 1920);
        assert_eq!(plan.active_outputs[1].mode.height, 1080);
        assert_eq!(plan.active_outputs[1].pos_x, 0);

        let cmd = plan.to_command_string();
        assert!(cmd.contains("--pos 0x0"));
        assert!(cmd.contains("--same-as"));
    }

    #[test]
    fn test_unused_monitors_turned_off() {
        let all = vec![
            make_output_with_mode("HDMI-1", 1920, 1080, 60.0),
            make_output_with_mode("eDP-1", 1920, 1080, 60.0),
            make_output_with_mode("DP-1", 2560, 1440, 60.0),
        ];

        let plan = create_plan(
            &all,
            &["edp".into()],
            Alignment::LeftToRight,
            false,
            None,
            true,
            true,
        ).unwrap();

        assert_eq!(plan.active_outputs.len(), 1);
        assert_eq!(plan.active_outputs[0].name, "eDP-1");
        assert_eq!(plan.turn_off_outputs, vec!["DP-1", "HDMI-1"]);

        let cmd = plan.to_command_string();
        assert!(cmd.contains("--output eDP-1 --mode 1920x1080 --rate 60.00 --pos 0x0 --primary"));
        assert!(cmd.contains("--output DP-1 --off"));
        assert!(cmd.contains("--output HDMI-1 --off"));
    }
}
