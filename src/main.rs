use std::env;
use std::process;
use xrandrx::cli::parse_args;
use xrandrx::executor::RealXrandrRunner;
use xrandrx::run_app;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let opts = match parse_args(args) {
        Ok(opts) => opts,
        Err(err) => {
            eprintln!("Error: {}", err);
            process::exit(1);
        }
    };

    let runner = RealXrandrRunner;
    if let Err(err) = run_app(&runner, opts) {
        eprintln!("Error: {}", err);
        process::exit(1);
    }
}
