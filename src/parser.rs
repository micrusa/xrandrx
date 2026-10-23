use crate::models::{Mode, Output};

/// Parses raw text output from `xrandr` / `xrandr --query`.
pub fn parse_xrandr_output(input: &str) -> Vec<Output> {
    let mut outputs = Vec::new();
    let mut current_output: Option<Output> = None;

    for line in input.lines() {
        if line.starts_with("Screen ") {
            continue;
        }

        // Output lines do not start with whitespace
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if let Some(out) = current_output.take() {
                outputs.push(out);
            }

            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() < 2 {
                continue;
            }

            let name = tokens[0].to_string();
            let mut connected = false;
            let mut is_primary = false;
            let mut current_mode = None;
            let mut current_pos = None;

            for &token in tokens.iter().skip(1) {
                if token == "connected" {
                    connected = true;
                } else if token == "primary" {
                    is_primary = true;
                } else if connected && token.contains('x') && token.contains('+') {
                    // Format: 1920x1080+0+0
                    let parts: Vec<&str> = token.split('+').collect();
                    if parts.len() == 3 {
                        current_mode = Some(parts[0].to_string());
                        if let (Ok(x), Ok(y)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
                            current_pos = Some((x, y));
                        }
                    }
                }
            }

            current_output = Some(Output {
                name,
                connected,
                is_primary,
                current_mode,
                current_pos,
                modes: Vec::new(),
            });
        } else if let Some(ref mut out) = current_output {
            if !out.connected {
                continue;
            }

            let line_trimmed = line.trim();
            if line_trimmed.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = line_trimmed.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            let res_token = tokens[0];
            if !res_token.contains('x') {
                continue;
            }

            let res_parts: Vec<&str> = res_token.split('x').collect();
            if res_parts.len() != 2 {
                continue;
            }

            let width: u32 = match res_parts[0].trim().parse() {
                Ok(w) => w,
                Err(_) => continue,
            };

            let height_str: String = res_parts[1].chars().filter(|c| c.is_ascii_digit()).collect();
            let height: u32 = match height_str.parse() {
                Ok(h) => h,
                Err(_) => continue,
            };

            for rate_token in &tokens[1..] {
                let is_current = rate_token.contains('*');
                let is_preferred = rate_token.contains('+');

                let rate_clean: String = rate_token
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();

                if let Ok(rate) = rate_clean.parse::<f64>() {
                    out.modes.push(Mode {
                        width,
                        height,
                        refresh_rate: rate,
                        is_preferred,
                        is_current,
                    });
                }
            }
        }
    }

    if let Some(out) = current_output {
        outputs.push(out);
    }

    outputs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let sample = r#"
Screen 0: minimum 320 x 200, current 3840 x 1080, maximum 16384 x 16384
eDP-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis) 344mm x 194mm
   1920x1080     60.05*+  48.04  
   1680x1050     60.05  
HDMI-1 connected 1920x1080+1920+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080     60.00*+  50.00    59.94  
   1280x720      60.00    50.00  
DP-1 disconnected (normal left inverted right x axis y axis)
"#;
        let outputs = parse_xrandr_output(sample);
        assert_eq!(outputs.len(), 3);

        assert_eq!(outputs[0].name, "eDP-1");
        assert!(outputs[0].connected);
        assert!(outputs[0].is_primary);
        assert_eq!(outputs[0].current_mode.as_deref(), Some("1920x1080"));
        assert_eq!(outputs[0].current_pos, Some((0, 0)));
        assert_eq!(outputs[0].modes.len(), 3);
        assert_eq!(outputs[0].modes[0].width, 1920);
        assert_eq!(outputs[0].modes[0].height, 1080);
        assert_eq!(outputs[0].modes[0].refresh_rate, 60.05);
        assert!(outputs[0].modes[0].is_current);
        assert!(outputs[0].modes[0].is_preferred);

        assert_eq!(outputs[1].name, "HDMI-1");
        assert!(outputs[1].connected);
        assert!(!outputs[1].is_primary);
        assert_eq!(outputs[1].current_pos, Some((1920, 0)));
        assert_eq!(outputs[1].modes.len(), 5);

        assert_eq!(outputs[2].name, "DP-1");
        assert!(!outputs[2].connected);
        assert!(outputs[2].modes.is_empty());
    }
}
