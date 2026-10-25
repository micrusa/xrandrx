use std::fs;
use std::path::{Path, PathBuf};
use crate::matcher::{get_output_family, match_output};
use crate::models::{Output, Plan, PlannedOutput};

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileOutput {
    pub name: String,
    pub mode: String,
    pub rate: Option<f64>,
    pub pos: (u32, u32),
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub name: String,
    pub mirror: bool,
    pub outputs: Vec<ProfileOutput>,
}

impl Profile {
    pub fn serialize(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("# xrandrx profile: {}\n", self.name));
        s.push_str(&format!("mirror = {}\n\n", self.mirror));
        for out in &self.outputs {
            s.push_str(&format!("[output.{}]\n", out.name));
            s.push_str(&format!("primary = {}\n", out.is_primary));
            s.push_str(&format!("mode = {}\n", out.mode));
            if let Some(rate) = out.rate {
                s.push_str(&format!("rate = {:.2}\n", rate));
            }
            s.push_str(&format!("pos = {}x{}\n\n", out.pos.0, out.pos.1));
        }
        s
    }
}

/// Parses the contents of a profile file.
pub fn parse_profile(content: &str, profile_name: &str) -> Result<Profile, String> {
    let mut mirror = false;
    let mut outputs = Vec::new();
    let mut current_output: Option<ProfileOutput> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(out) = current_output.take() {
                outputs.push(out);
            }

            let header = &trimmed[1..trimmed.len() - 1].trim();
            if header.eq_ignore_ascii_case("profile") || header.eq_ignore_ascii_case("settings") {
                continue;
            }

            let name = if header.to_ascii_lowercase().starts_with("output.") {
                header["output.".len()..].trim().to_string()
            } else {
                header.to_string()
            };

            current_output = Some(ProfileOutput {
                name,
                mode: String::new(),
                rate: None,
                pos: (0, 0),
                is_primary: false,
            });
        } else if let Some(eq_idx) = trimmed.find('=') {
            let key = trimmed[..eq_idx].trim().to_ascii_lowercase();
            let val = trimmed[eq_idx + 1..].trim();

            if let Some(ref mut out) = current_output {
                match key.as_str() {
                    "primary" => {
                        out.is_primary = val.eq_ignore_ascii_case("true");
                    }
                    "mode" => {
                        out.mode = val.to_string();
                    }
                    "rate" => {
                        out.rate = val.parse::<f64>().ok();
                    }
                    "pos" => {
                        let cleaned = val.replace(',', "x").replace('+', "x");
                        let parts: Vec<&str> = cleaned.split('x').filter(|s| !s.is_empty()).collect();
                        if parts.len() >= 2 {
                            if let (Ok(x), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                                out.pos = (x, y);
                            }
                        }
                    }
                    _ => {}
                }
            } else if key == "mirror" {
                mirror = val.eq_ignore_ascii_case("true");
            }
        }
    }

    if let Some(out) = current_output {
        outputs.push(out);
    }

    Ok(Profile {
        name: profile_name.to_string(),
        mirror,
        outputs,
    })
}

/// Returns the path to the profiles directory.
/// Checks `XRANDRX_PROFILES_DIR`, then `$XDG_CONFIG_HOME/xrandrx/profiles`,
/// and defaults to `~/.config/xrandrx/profiles`.
pub fn get_profiles_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XRANDRX_PROFILES_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("xrandrx").join("profiles")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join("xrandrx").join("profiles")
    } else {
        PathBuf::from(".config/xrandrx/profiles")
    }
}

/// Computes the Levenshtein distance between two strings.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0; b_chars.len() + 1]; a_chars.len() + 1];

    for i in 0..=a_chars.len() {
        d[i][0] = i;
    }
    for j in 0..=b_chars.len() {
        d[0][j] = j;
    }

    for i in 1..=a_chars.len() {
        for j in 1..=b_chars.len() {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
        }
    }
    d[a_chars.len()][b_chars.len()]
}

/// Finds the closest matching profile name from candidates.
/// Ranking:
/// 1. Exact match (case-insensitive)
/// 2. Prefix match (e.g. query "myp" matches "myprofilename", shorter suffix wins)
/// 3. Candidate is prefix of query
/// 4. Substring match
/// 5. Levenshtein edit distance
pub fn find_closest_profile_name(candidates: &[String], query: &str) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }

    let q = query.trim().to_ascii_lowercase();

    let mut ranked: Vec<((u8, usize, usize, String), &String)> = candidates
        .iter()
        .map(|cand| {
            let c = cand.trim().to_ascii_lowercase();
            let score = if c == q {
                (0, 0, cand.len(), cand.clone())
            } else if c.starts_with(&q) {
                (1, c.len() - q.len(), cand.len(), cand.clone())
            } else if q.starts_with(&c) {
                (2, q.len() - c.len(), cand.len(), cand.clone())
            } else if c.contains(&q) {
                (3, c.len() - q.len(), cand.len(), cand.clone())
            } else {
                let dist = levenshtein_distance(&q, &c);
                (4, dist, cand.len(), cand.clone())
            };
            (score, cand)
        })
        .collect();

    ranked.sort_by(|a, b| a.0.cmp(&b.0));
    ranked.first().map(|(_, cand)| (*cand).clone())
}

/// Saves the currently active xrandr configuration as a profile.
pub fn save_current_profile(
    outputs: &[Output],
    profile_name: &str,
    dir: &Path,
) -> Result<PathBuf, String> {
    let active_outputs: Vec<&Output> = outputs
        .iter()
        .filter(|o| o.connected && o.current_mode.is_some())
        .collect();

    if active_outputs.is_empty() {
        return Err("No active displays currently configured to save into a profile".to_string());
    }

    // Determine if setup is mirrored
    let mirror = if active_outputs.len() >= 2 {
        let first_pos = active_outputs[0].current_pos.unwrap_or((0, 0));
        let first_mode = active_outputs[0].current_mode.as_deref();
        active_outputs.iter().all(|o| {
            o.current_pos.unwrap_or((0, 0)) == first_pos && o.current_mode.as_deref() == first_mode
        })
    } else {
        false
    };

    let mut profile_outputs = Vec::new();
    for out in &active_outputs {
        let mode = out.current_mode.clone().unwrap();
        let rate = out
            .modes
            .iter()
            .find(|m| m.is_current)
            .map(|m| m.refresh_rate);
        let pos = out.current_pos.unwrap_or((0, 0));
        let is_primary = out.is_primary;

        profile_outputs.push(ProfileOutput {
            name: out.name.clone(),
            mode,
            rate,
            pos,
            is_primary,
        });
    }

    let profile = Profile {
        name: profile_name.to_string(),
        mirror,
        outputs: profile_outputs,
    };

    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create profiles directory '{}': {}", dir.display(), e))?;

    let file_path = dir.join(profile_name);
    fs::write(&file_path, profile.serialize())
        .map_err(|e| format!("Failed to write profile to '{}': {}", file_path.display(), e))?;

    Ok(file_path)
}

/// Lists all profile names present in the profiles directory.
pub fn list_profile_names(dir: &Path) -> Vec<String> {
    if !dir.exists() {
        return Vec::new();
    }
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') {
                        names.push(name);
                    }
                }
            }
        }
    }
    names.sort();
    names
}

/// Loads all valid profiles from the profiles directory.
pub fn load_all_profiles(dir: &Path) -> Result<Vec<Profile>, String> {
    let names = list_profile_names(dir);
    if names.is_empty() {
        return Err(format!("No profiles found in '{}'", dir.display()));
    }

    let mut profiles = Vec::new();
    for name in names {
        let file_path = dir.join(&name);
        if let Ok(content) = fs::read_to_string(&file_path) {
            if let Ok(profile) = parse_profile(&content, &name) {
                profiles.push(profile);
            }
        }
    }

    if profiles.is_empty() {
        return Err(format!("No valid profiles found in '{}'", dir.display()));
    }

    Ok(profiles)
}

/// Selects the best matching profile based on connected displays and mirror constraints.
/// Ranking priority:
/// 1. Extended displays prioritised over mirror (unless `mirror_only` is true, which strictly requires mirror)
/// 2. Monitor count match (e.g. 2 monitors matching profile with 2 monitors)
/// 3. Output connector type match (hdmi, dp, edp, etc.)
/// 4. Mode resolution and refresh rate match
pub fn select_best_profile<'a>(
    profiles: &'a [Profile],
    connected_outputs: &[&Output],
    mirror_only: bool,
) -> Result<&'a Profile, String> {
    let candidates: Vec<&Profile> = if mirror_only {
        profiles.iter().filter(|p| p.mirror).collect()
    } else {
        profiles.iter().collect()
    };

    if candidates.is_empty() {
        if mirror_only {
            return Err("No mirroring profiles found for --auto-profile -m".to_string());
        } else {
            return Err("No profiles found to auto-select from".to_string());
        }
    }

    let n = connected_outputs.len();
    let connected_cloned: Vec<Output> = connected_outputs.iter().map(|&o| o.clone()).collect();
    let connected_families: Vec<String> = connected_outputs
        .iter()
        .map(|o| get_output_family(&o.name))
        .collect();

    let mut scored: Vec<((u8, usize, usize, u64, String), &Profile)> = candidates
        .into_iter()
        .map(|p| {
            // 1. Mirror priority (if mirror_only is false, extended is prioritised over mirror)
            let mirror_penalty = if mirror_only {
                0
            } else if p.mirror {
                1
            } else {
                0
            };

            // 2. Monitor count match (exact count match = 0 diff)
            let count_diff = if p.outputs.len() == n {
                0
            } else {
                (p.outputs.len() as isize - n as isize).unsigned_abs() + 1
            };

            // 3. Output type (connector family) match
            let mut available_families = connected_families.clone();
            let mut matching_families = 0;
            for p_out in &p.outputs {
                let fam = get_output_family(&p_out.name);
                if let Some(pos) = available_families.iter().position(|f| *f == fam) {
                    available_families.remove(pos);
                    matching_families += 1;
                }
            }
            let type_mismatch = n.saturating_sub(matching_families);

            // 4. Resolution and refresh rate match
            let mut res_rate_penalty: u64 = 0;
            for p_out in &p.outputs {
                if let Ok(matched) = match_output(&p_out.name, &connected_cloned) {
                    let has_mode = matched.modes.iter().any(|m| m.mode_str() == p_out.mode);
                    if !has_mode {
                        res_rate_penalty += 1000;
                    }
                    if let Some(pref) = matched.modes.iter().find(|m| m.is_preferred) {
                        if pref.mode_str() != p_out.mode {
                            res_rate_penalty += 100;
                        }
                        if let Some(target_rate) = p_out.rate {
                            let diff = (pref.refresh_rate - target_rate).abs() as u64;
                            res_rate_penalty += diff;
                        }
                    }
                } else {
                    res_rate_penalty += 2000;
                }
            }

            let score = (mirror_penalty, count_diff, type_mismatch, res_rate_penalty, p.name.clone());
            (score, p)
        })
        .collect();

    scored.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(scored[0].1)
}

/// Automatically selects and builds a plan for the best matching profile.
pub fn select_and_load_auto_profile(
    all_outputs: &[Output],
    mirror_only: bool,
    off_unused: bool,
    dir: &Path,
) -> Result<(Plan, String), String> {
    let profiles = load_all_profiles(dir)?;
    let connected: Vec<&Output> = all_outputs.iter().filter(|o| o.connected).collect();
    if connected.is_empty() {
        return Err("No connected displays found to auto-select profile".to_string());
    }

    let best_profile = select_best_profile(&profiles, &connected, mirror_only)?;
    let plan = build_plan_from_profile(best_profile, all_outputs, off_unused)?;
    Ok((plan, best_profile.name.clone()))
}

/// Loads a profile by name from disk (supporting closest match) and creates an execution Plan.
pub fn load_profile_plan(
    all_outputs: &[Output],
    profile_query: &str,
    off_unused: bool,
    dir: &Path,
) -> Result<(Plan, String), String> {
    let available_names = list_profile_names(dir);
    if available_names.is_empty() {
        return Err(format!("No profiles found in '{}'", dir.display()));
    }

    let matched_name = find_closest_profile_name(&available_names, profile_query).ok_or_else(|| {
        format!("No profile matching '{}' found in '{}'", profile_query, dir.display())
    })?;

    let file_path = dir.join(&matched_name);
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read profile '{}': {}", file_path.display(), e))?;

    let profile = parse_profile(&content, &matched_name)?;
    let plan = build_plan_from_profile(&profile, all_outputs, off_unused)?;
    Ok((plan, matched_name))
}

/// Builds an execution plan from a Profile against the current connected displays.
/// Loading is flexible: if a resolution is missing, the preferred one is chosen.
pub fn build_plan_from_profile(
    profile: &Profile,
    all_outputs: &[Output],
    off_unused: bool,
) -> Result<Plan, String> {
    let connected_outputs: Vec<&Output> = all_outputs.iter().filter(|o| o.connected).collect();
    if connected_outputs.is_empty() {
        return Err("No connected displays found to apply profile".to_string());
    }

    let connected_cloned: Vec<Output> = connected_outputs.iter().map(|&o| o.clone()).collect();
    let mut matched_entries = Vec::new();

    for p_out in &profile.outputs {
        if let Ok(matched) = match_output(&p_out.name, &connected_cloned) {
            let actual_output = connected_outputs
                .iter()
                .find(|o| o.name == matched.name)
                .expect("output exists");

            // Flexible mode selection:
            // 1. Try to find the exact resolution specified in the profile.
            // 2. If missing, choose the preferred mode (`is_preferred == true`), or best mode.
            let mode = if let Some(exact_mode) = actual_output
                .modes
                .iter()
                .filter(|m| m.mode_str() == p_out.mode)
                .min_by(|a, b| {
                    if let Some(target_rate) = p_out.rate {
                        let diff_a = (a.refresh_rate - target_rate).abs();
                        let diff_b = (b.refresh_rate - target_rate).abs();
                        diff_a.partial_cmp(&diff_b).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        b.refresh_rate.partial_cmp(&a.refresh_rate).unwrap_or(std::cmp::Ordering::Equal)
                    }
                })
            {
                exact_mode.clone()
            } else {
                // Resolution is missing: fall back to preferred mode!
                let preferred_mode = actual_output
                    .modes
                    .iter()
                    .filter(|m| m.is_preferred)
                    .max_by(|a, b| {
                        a.refresh_rate.partial_cmp(&b.refresh_rate).unwrap_or(std::cmp::Ordering::Equal)
                    });
                preferred_mode
                    .cloned()
                    .or_else(|| actual_output.best_mode().cloned())
                    .ok_or_else(|| format!("Display '{}' has no available modes", actual_output.name))?
            };

            matched_entries.push((*actual_output, p_out, mode));
        }
    }

    if matched_entries.is_empty() {
        return Err(format!(
            "None of the displays configured in profile '{}' are currently connected",
            profile.name
        ));
    }

    // Determine primary display:
    // If any matched entry was marked primary in the profile, pick that one.
    // Otherwise default to the first matched entry.
    let primary_idx = matched_entries
        .iter()
        .position(|(_, p_out, _)| p_out.is_primary)
        .unwrap_or(0);
    let primary_name = matched_entries[primary_idx].0.name.clone();

    let mut active_outputs = Vec::new();

    if profile.mirror {
        for (i, (out, _, mode)) in matched_entries.into_iter().enumerate() {
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
    } else {
        // Extended mode:
        // Use saved positions
        for (i, (out, p_out, mode)) in matched_entries.into_iter().enumerate() {
            let is_primary = i == primary_idx;
            active_outputs.push(PlannedOutput {
                name: out.name.clone(),
                mode,
                pos_x: p_out.pos.0,
                pos_y: p_out.pos.1,
                is_primary,
                same_as: None,
            });
        }
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
    use crate::models::Mode;

    #[test]
    fn test_find_closest_profile_exact_and_prefix() {
        let profiles = vec![
            "myprofilename".to_string(),
            "work".to_string(),
            "docked".to_string(),
        ];

        // Exact match
        assert_eq!(
            find_closest_profile_name(&profiles, "work").as_deref(),
            Some("work")
        );

        // Prefix match: "myp" should choose "myprofilename"
        assert_eq!(
            find_closest_profile_name(&profiles, "myp").as_deref(),
            Some("myprofilename")
        );
    }

    #[test]
    fn test_find_closest_profile_closer_prefix() {
        let profiles = vec![
            "myprofilename".to_string(),
            "myp2".to_string(),
            "work".to_string(),
        ];

        // "myp" has closer prefix length to "myp2" than "myprofilename"
        assert_eq!(
            find_closest_profile_name(&profiles, "myp").as_deref(),
            Some("myp2")
        );
    }

    #[test]
    fn test_find_closest_profile_levenshtein_typo() {
        let profiles = vec![
            "myprofilename".to_string(),
            "work".to_string(),
            "docked".to_string(),
        ];

        // Typo: "myprofle" matches "myprofilename"
        assert_eq!(
            find_closest_profile_name(&profiles, "myprofle").as_deref(),
            Some("myprofilename")
        );
    }

    #[test]
    fn test_serialize_and_parse_profile() {
        let profile = Profile {
            name: "test_profile".to_string(),
            mirror: false,
            outputs: vec![
                ProfileOutput {
                    name: "eDP-1".to_string(),
                    mode: "1920x1080".to_string(),
                    rate: Some(144.0),
                    pos: (0, 0),
                    is_primary: true,
                },
                ProfileOutput {
                    name: "HDMI-1".to_string(),
                    mode: "2560x1440".to_string(),
                    rate: Some(60.0),
                    pos: (1920, 0),
                    is_primary: false,
                },
            ],
        };

        let serialized = profile.serialize();
        let parsed = parse_profile(&serialized, "test_profile").unwrap();

        assert_eq!(parsed.name, "test_profile");
        assert!(!parsed.mirror);
        assert_eq!(parsed.outputs.len(), 2);
        assert_eq!(parsed.outputs[0].name, "eDP-1");
        assert_eq!(parsed.outputs[0].mode, "1920x1080");
        assert_eq!(parsed.outputs[0].rate, Some(144.0));
        assert_eq!(parsed.outputs[0].pos, (0, 0));
        assert!(parsed.outputs[0].is_primary);

        assert_eq!(parsed.outputs[1].name, "HDMI-1");
        assert_eq!(parsed.outputs[1].mode, "2560x1440");
        assert_eq!(parsed.outputs[1].rate, Some(60.0));
        assert_eq!(parsed.outputs[1].pos, (1920, 0));
        assert!(!parsed.outputs[1].is_primary);
    }

    #[test]
    fn test_load_profile_missing_resolution_fallback_to_preferred() {
        let connected = vec![
            Output {
                name: "HDMI-1".to_string(),
                connected: true,
                is_primary: false,
                current_mode: None,
                current_pos: None,
                modes: vec![
                    Mode {
                        width: 3840,
                        height: 2160,
                        refresh_rate: 60.0,
                        is_preferred: false,
                        is_current: false,
                    },
                    Mode {
                        width: 2560,
                        height: 1440,
                        refresh_rate: 144.0,
                        is_preferred: true, // preferred mode!
                        is_current: false,
                    },
                    Mode {
                        width: 1920,
                        height: 1080,
                        refresh_rate: 60.0,
                        is_preferred: false,
                        is_current: false,
                    },
                ],
            },
        ];

        // Profile specifies a 1280x720 mode which HDMI-1 does NOT support
        let profile = Profile {
            name: "missing_res".to_string(),
            mirror: false,
            outputs: vec![ProfileOutput {
                name: "hdmi".to_string(),
                mode: "1280x720".to_string(),
                rate: Some(60.0),
                pos: (0, 0),
                is_primary: true,
            }],
        };

        let plan = build_plan_from_profile(&profile, &connected, true).unwrap();
        assert_eq!(plan.active_outputs.len(), 1);
        // Preferred 2560x1440 @ 144.00Hz chosen!
        assert_eq!(plan.active_outputs[0].mode.width, 2560);
        assert_eq!(plan.active_outputs[0].mode.height, 1440);
        assert_eq!(plan.active_outputs[0].mode.refresh_rate, 144.0);
    }

    #[test]
    fn test_auto_profile_selection_criteria() {
        let o_hdmi = Output {
            name: "HDMI-1".to_string(),
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
        };
        let o_edp = Output {
            name: "eDP-1".to_string(),
            connected: true,
            is_primary: false,
            current_mode: None,
            current_pos: None,
            modes: vec![Mode {
                width: 1920,
                height: 1080,
                refresh_rate: 144.0,
                is_preferred: true,
                is_current: false,
            }],
        };
        let connected = vec![&o_hdmi, &o_edp];

        // 1. Monitor count match: single display vs dual displays
        let p_single = Profile {
            name: "single_monitor".to_string(),
            mirror: false,
            outputs: vec![ProfileOutput {
                name: "edp".to_string(),
                mode: "1920x1080".to_string(),
                rate: Some(144.0),
                pos: (0, 0),
                is_primary: true,
            }],
        };
        let p_dual_wrong_type = Profile {
            name: "dual_dp".to_string(),
            mirror: false,
            outputs: vec![
                ProfileOutput {
                    name: "dp0".to_string(),
                    mode: "1920x1080".to_string(),
                    rate: Some(60.0),
                    pos: (0, 0),
                    is_primary: true,
                },
                ProfileOutput {
                    name: "dp1".to_string(),
                    mode: "1920x1080".to_string(),
                    rate: Some(60.0),
                    pos: (1920, 0),
                    is_primary: false,
                },
            ],
        };
        let list1 = [p_single.clone(), p_dual_wrong_type.clone()];
        let best_by_count = select_best_profile(&list1, &connected, false).unwrap();
        assert_eq!(best_by_count.name, "dual_dp");

        // 2. Output type match: dual dp vs dual hdmi+edp
        let p_dual_matching_type = Profile {
            name: "dual_hdmi_edp".to_string(),
            mirror: false,
            outputs: vec![
                ProfileOutput {
                    name: "hdmi".to_string(),
                    mode: "1920x1080".to_string(),
                    rate: Some(60.0),
                    pos: (0, 0),
                    is_primary: false,
                },
                ProfileOutput {
                    name: "edp".to_string(),
                    mode: "1920x1080".to_string(),
                    rate: Some(144.0),
                    pos: (1920, 0),
                    is_primary: true,
                },
            ],
        };
        let list2 = [p_dual_wrong_type, p_dual_matching_type.clone()];
        let best_by_type = select_best_profile(&list2, &connected, false).unwrap();
        assert_eq!(best_by_type.name, "dual_hdmi_edp");

        // 3. Extended prioritised over mirror unless -m is set
        let p_dual_mirror = Profile {
            name: "dual_hdmi_edp_mirror".to_string(),
            mirror: true,
            outputs: vec![
                ProfileOutput {
                    name: "hdmi".to_string(),
                    mode: "1920x1080".to_string(),
                    rate: Some(60.0),
                    pos: (0, 0),
                    is_primary: false,
                },
                ProfileOutput {
                    name: "edp".to_string(),
                    mode: "1920x1080".to_string(),
                    rate: Some(60.0),
                    pos: (0, 0),
                    is_primary: true,
                },
            ],
        };

        // Without -m: extended profile wins
        let list3 = [p_dual_mirror.clone(), p_dual_matching_type.clone()];
        let best_no_m = select_best_profile(&list3, &connected, false).unwrap();
        assert_eq!(best_no_m.name, "dual_hdmi_edp");

        // With -m: mirror profile wins
        let list4 = [p_dual_mirror.clone(), p_dual_matching_type];
        let best_with_m = select_best_profile(&list4, &connected, true).unwrap();
        assert_eq!(best_with_m.name, "dual_hdmi_edp_mirror");
    }
}
