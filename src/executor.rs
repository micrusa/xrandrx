use std::process::Command;

pub trait XrandrRunner {
    fn query(&self) -> Result<String, String>;
    fn apply(&self, args: &[String]) -> Result<(), String>;
}

pub struct RealXrandrRunner;

impl XrandrRunner for RealXrandrRunner {
    fn query(&self) -> Result<String, String> {
        let output = Command::new("xrandr")
            .arg("--query")
            .output()
            .map_err(|e| format!("Failed to execute 'xrandr --query': {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("xrandr --query failed with status {}: {}", output.status, stderr));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| format!("Failed to parse xrandr output as UTF-8: {}", e))
    }

    fn apply(&self, args: &[String]) -> Result<(), String> {
        let status = Command::new("xrandr")
            .args(args)
            .status()
            .map_err(|e| format!("Failed to execute xrandr command: {}", e))?;

        if !status.success() {
            return Err(format!("xrandr failed with exit code {}", status));
        }

        Ok(())
    }
}

pub struct DryRunXrandrRunner<'a> {
    pub inner: &'a dyn XrandrRunner,
}

impl<'a> XrandrRunner for DryRunXrandrRunner<'a> {
    fn query(&self) -> Result<String, String> {
        self.inner.query()
    }

    fn apply(&self, args: &[String]) -> Result<(), String> {
        println!("[dry-run] Would execute: xrandr {}", args.join(" "));
        Ok(())
    }
}

pub struct MockXrandrRunner {
    pub mock_query_output: String,
}

impl XrandrRunner for MockXrandrRunner {
    fn query(&self) -> Result<String, String> {
        Ok(self.mock_query_output.clone())
    }

    fn apply(&self, _args: &[String]) -> Result<(), String> {
        Ok(())
    }
}
