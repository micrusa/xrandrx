use xrandrx::cli::parse_args;
use xrandrx::executor::MockXrandrRunner;
use xrandrx::run_app;

const FIXTURE_3_MONITORS: &str = r#"
Screen 0: minimum 320 x 200, current 5760 x 1080, maximum 16384 x 16384
HDMI-1 connected 1920x1080+0+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080     60.00*+  50.00    59.94  
   1280x720      60.00  
eDP-1 connected primary 1920x1080+1920+0 (normal left inverted right x axis y axis) 344mm x 194mm
   1920x1080     144.00*+ 60.05    48.04  
   1680x1050     60.05  
DP-1 connected 1920x1080+3840+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080     60.00*+  50.00  
   1280x720      60.00  
VGA-1 disconnected (normal left inverted right x axis y axis)
"#;

const FIXTURE_4_MONITORS: &str = r#"
Screen 0: minimum 8 x 8, current 8320 x 2160, maximum 32767 x 32767
DP-0 connected 1920x1080+0+0 (normal left inverted right x axis y axis) 531mm x 298mm
   1920x1080     60.00*+  50.00  
DP-1 connected 1920x1080+1920+0 (normal left inverted right x axis y axis) 531mm x 298mm
   1920x1080     60.00*+  50.00  
DP-2 connected 2560x1440+3840+0 (normal left inverted right x axis y axis) 597mm x 336mm
   2560x1440     144.00*+ 60.00  
   1920x1080     60.00  
DP-3 connected 1920x1080+6400+0 (normal left inverted right x axis y axis) 531mm x 298mm
   1920x1080     60.00*+  50.00  
"#;

const FIXTURE_HYBRID_HDMI01: &str = r#"
Screen 0: minimum 8 x 8, current 3840 x 1080, maximum 32767 x 32767
eDP-1-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis) 344mm x 193mm
   1920x1080    144.00 +  60.00   144.00* 
HDMI-0-1 connected 1920x1080+1920+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080     75.00*+  60.00  
DP-1 disconnected (normal left inverted right x axis y axis)
"#;

const FIXTURE_MULTI_HDMI: &str = r#"
Screen 0: minimum 8 x 8, current 5760 x 1080, maximum 32767 x 32767
HDMI-1 connected 1920x1080+0+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080     60.00*+  50.00  
HDMI-2 connected 1920x1080+1920+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080     75.00*+  60.00  
DP-1 connected 2560x1440+3840+0 (normal left inverted right x axis y axis) 597mm x 336mm
   2560x1440     144.00*+ 60.00  
"#;

#[test]
fn test_equivalence_default_primary_and_best() {
    // "For example: xrandrx --ltr hdmi edp dp == xrandrx --ltr hdmi edp dp --primary edp --best"
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_3_MONITORS.to_string(),
    };

    let opts1 = parse_args(vec!["--ltr", "hdmi", "edp", "dp", "--dry-run"]).unwrap();
    let cmd1 = run_app(&runner, opts1).unwrap();

    let opts2 = parse_args(vec![
        "--ltr", "hdmi", "edp", "dp", "--primary", "edp", "--best", "--dry-run",
    ])
    .unwrap();
    let cmd2 = run_app(&runner, opts2).unwrap();

    assert_eq!(cmd1, cmd2);

    // Verify positioning and flags in the command:
    // HDMI-1 at 0x0
    // eDP-1 at 1920x0 (primary, 144Hz)
    // DP-1 at 3840x0
    assert!(cmd1.contains("--output HDMI-1 --mode 1920x1080 --rate 60.00 --pos 0x0"));
    assert!(cmd1.contains("--output eDP-1 --mode 1920x1080 --rate 144.00 --pos 1920x0 --primary"));
    assert!(cmd1.contains("--output DP-1 --mode 1920x1080 --rate 60.00 --pos 3840x0"));
}

#[test]
fn test_four_screens_center_highest_resolution() {
    // "if there are 4, choose the highest res from the two centered ones"
    // DP-0 (1080p), DP-1 (1080p), DP-2 (1440p), DP-3 (1080p)
    // Centered ones are DP-1 and DP-2. DP-2 is 1440p, so DP-2 should be primary.
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_4_MONITORS.to_string(),
    };

    let opts = parse_args(vec!["--ltr", "dp0", "dp1", "dp2", "dp3", "--dry-run"]).unwrap();
    let cmd = run_app(&runner, opts).unwrap();

    assert!(cmd.contains("--output DP-2 --mode 2560x1440 --rate 144.00 --pos 3840x0 --primary"));
    assert!(!cmd.contains("--output DP-1 --mode 1920x1080 --rate 60.00 --pos 1920x0 --primary"));
}

#[test]
fn test_rtl_alignment() {
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_3_MONITORS.to_string(),
    };

    // RTL: hdmi, edp, dp
    // Placed Right-to-Left, so visual layout is: DP-1 (leftmost), eDP-1 (center), HDMI-1 (rightmost)
    let opts = parse_args(vec!["--rtl", "hdmi", "edp", "dp", "--dry-run"]).unwrap();
    let cmd = run_app(&runner, opts).unwrap();

    assert!(cmd.contains("--output DP-1 --mode 1920x1080 --rate 60.00 --pos 0x0"));
    assert!(cmd.contains("--output eDP-1 --mode 1920x1080 --rate 144.00 --pos 1920x0 --primary"));
    assert!(cmd.contains("--output HDMI-1 --mode 1920x1080 --rate 60.00 --pos 3840x0"));
}

#[test]
fn test_single_hdmi_alias_hdmi01() {
    // Single HDMI port named HDMI-0-1 matched simply by "hdmi"
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_HYBRID_HDMI01.to_string(),
    };

    let opts = parse_args(vec!["--ltr", "edp", "hdmi", "--dry-run"]).unwrap();
    let cmd = run_app(&runner, opts).unwrap();

    assert!(cmd.contains("--output eDP-1-1 --mode 1920x1080 --rate 144.00 --pos 0x0"));
    assert!(cmd.contains("--output HDMI-0-1 --mode 1920x1080 --rate 75.00 --pos 1920x0"));
}

#[test]
fn test_multi_hdmi_enumeration() {
    // HDMI-1 and HDMI-2 present
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_MULTI_HDMI.to_string(),
    };

    // "hdmi" selects first one (HDMI-1)
    let opts_first = parse_args(vec!["--ltr", "hdmi", "dp", "--dry-run"]).unwrap();
    let cmd_first = run_app(&runner, opts_first).unwrap();
    assert!(cmd_first.contains("--output HDMI-1"));
    assert!(cmd_first.contains("--output HDMI-2 --off"));

    // "hdmi1" selects second one (HDMI-2)
    let opts_second = parse_args(vec!["--ltr", "hdmi1", "dp", "--dry-run"]).unwrap();
    let cmd_second = run_app(&runner, opts_second).unwrap();
    assert!(cmd_second.contains("--output HDMI-2"));
    assert!(cmd_second.contains("--output HDMI-1 --off"));
}

#[test]
fn test_list_displays() {
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_3_MONITORS.to_string(),
    };

    let opts = parse_args(vec!["--list"]).unwrap();
    let result = run_app(&runner, opts).unwrap();

    assert!(result.contains("HDMI-1"));
    assert!(result.contains("eDP-1"));
    assert!(result.contains("DP-1"));
    assert!(result.contains("Disconnected:"));
    assert!(result.contains("VGA-1"));
}

#[test]
fn test_best_picks_highest_res_and_highest_rate() {
    // Check that best mode prefers 2560x1440 @ 144Hz over 2560x1440 @ 60Hz and 1080p
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_MULTI_HDMI.to_string(),
    };

    let opts = parse_args(vec!["dp", "--dry-run"]).unwrap();
    let cmd = run_app(&runner, opts).unwrap();

    assert!(cmd.contains("--output DP-1 --mode 2560x1440 --rate 144.00 --pos 0x0 --primary"));
}

#[test]
fn test_explicit_primary_override() {
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_3_MONITORS.to_string(),
    };

    // Explicitly designate hdmi as primary instead of center (edp)
    let opts = parse_args(vec!["--ltr", "hdmi", "edp", "dp", "--primary", "hdmi", "--dry-run"]).unwrap();
    let cmd = run_app(&runner, opts).unwrap();

    assert!(cmd.contains("--output HDMI-1 --mode 1920x1080 --rate 60.00 --pos 0x0 --primary"));
    assert!(!cmd.contains("--output eDP-1 --mode 1920x1080 --rate 144.00 --pos 1920x0 --primary"));
}

#[test]
fn test_error_on_unknown_monitor() {
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_3_MONITORS.to_string(),
    };

    let opts = parse_args(vec!["--ltr", "hdmi", "nonexistent", "--dry-run"]).unwrap();
    let err = run_app(&runner, opts).unwrap_err();

    assert!(err.contains("Could not match monitor selector 'nonexistent'"));
}

#[test]
fn test_error_on_duplicate_monitor() {
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_3_MONITORS.to_string(),
    };

    let opts = parse_args(vec!["--ltr", "hdmi", "hdmi", "--dry-run"]).unwrap();
    let err = run_app(&runner, opts).unwrap_err();

    assert!(err.contains("specified more than once"));
}

#[test]
fn test_keep_others_flag() {
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_3_MONITORS.to_string(),
    };

    let opts = parse_args(vec!["--ltr", "edp", "--keep-others", "--dry-run"]).unwrap();
    let cmd = run_app(&runner, opts).unwrap();

    assert!(cmd.contains("--output eDP-1"));
    assert!(!cmd.contains("--off"));
}

#[test]
fn test_mirror_mode() {
    let runner = MockXrandrRunner {
        mock_query_output: FIXTURE_3_MONITORS.to_string(),
    };

    let opts = parse_args(vec!["--mirror", "hdmi", "edp", "--dry-run"]).unwrap();
    let cmd = run_app(&runner, opts).unwrap();

    assert!(cmd.contains("--output HDMI-1 --mode 1920x1080 --rate 60.00 --pos 0x0"));
    assert!(cmd.contains("--output eDP-1 --mode 1920x1080 --rate 144.00 --pos 0x0"));
    assert!(cmd.contains("--same-as"));
    assert!(cmd.contains("--output DP-1 --off"));
}
