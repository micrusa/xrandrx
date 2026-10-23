#[derive(Debug, Clone, PartialEq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f64,
    pub is_preferred: bool,
    pub is_current: bool,
}

impl Mode {
    pub fn resolution_pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn mode_str(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub name: String,
    pub connected: bool,
    pub is_primary: bool,
    pub current_mode: Option<String>,
    pub current_pos: Option<(u32, u32)>,
    pub modes: Vec<Mode>,
}

impl Output {
    /// Returns the best mode for this output:
    /// Highest resolution (width * height), and highest refresh rate for that resolution.
    pub fn best_mode(&self) -> Option<&Mode> {
        self.modes.iter().max_by(|a, b| {
            let res_a = a.resolution_pixels();
            let res_b = b.resolution_pixels();
            if res_a != res_b {
                return res_a.cmp(&res_b);
            }
            a.refresh_rate.partial_cmp(&b.refresh_rate).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    LeftToRight, // --ltr
    RightToLeft, // --rtl
}

impl Default for Alignment {
    fn default() -> Self {
        Alignment::LeftToRight
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedOutput {
    pub name: String,
    pub mode: Mode,
    pub pos_x: u32,
    pub pos_y: u32,
    pub is_primary: bool,
    pub same_as: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub active_outputs: Vec<PlannedOutput>,
    pub turn_off_outputs: Vec<String>,
}

impl Plan {
    /// Generate the argument list for `xrandr`
    pub fn to_xrandr_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        for planned in &self.active_outputs {
            args.push("--output".to_string());
            args.push(planned.name.clone());
            args.push("--mode".to_string());
            args.push(planned.mode.mode_str());
            args.push("--rate".to_string());
            // Round or format refresh rate nicely (e.g. 60.00 -> 60.00)
            args.push(format!("{:.2}", planned.mode.refresh_rate));
            args.push("--pos".to_string());
            args.push(format!("{}x{}", planned.pos_x, planned.pos_y));

            if let Some(ref same_as) = planned.same_as {
                args.push("--same-as".to_string());
                args.push(same_as.clone());
            }

            if planned.is_primary {
                args.push("--primary".to_string());
            }
        }

        for off in &self.turn_off_outputs {
            args.push("--output".to_string());
            args.push(off.clone());
            args.push("--off".to_string());
        }

        args
    }

    /// Generate human-readable command string
    pub fn to_command_string(&self) -> String {
        let args = self.to_xrandr_args();
        format!("xrandr {}", args.join(" "))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CliOptions {
    pub screens: Vec<String>,
    pub alignment: Alignment,
    pub mirror: bool,
    pub primary: Option<String>,
    pub best: bool,
    pub dry_run: bool,
    pub list: bool,
    pub off_unused: bool,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            screens: Vec::new(),
            alignment: Alignment::LeftToRight,
            mirror: false,
            primary: None,
            best: true, // User requested: "option --best for best quality, which will be default"
            dry_run: false,
            list: false,
            off_unused: true,
        }
    }
}
