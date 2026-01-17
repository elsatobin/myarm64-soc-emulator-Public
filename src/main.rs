use std::fs;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

use arm64_soc_emulator::system::{Soc, SocConfig};

/// arm64 SoC emulator
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// binary file to execute
    binary: PathBuf,

    /// entry point address (hex)
    #[arg(short, long, default_value = "0x40000000", value_parser = parse_hex)]
    entry: u64,

    /// ram size in mb
    #[arg(short, long, default_value_t = 64)]
    ram: usize,

    /// maximum instructions to execute (0 = unlimited)
    #[arg(short, long, default_value_t = 0)]
    max: u64,

    /// xump cpu state after execution
    #[arg(short, long)]
    dump: bool,
}

fn parse_hex(s: &str) -> Result<u64, String> {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(s, 16).map_err(|_| format!("invalid hex address: {s}"))
}

fn main() -> ExitCode {
    FmtSubscriber::builder().with_max_level(Level::INFO).with_target(false).compact().init();

    let args = Args::parse();

    let binary = match fs::read(&args.binary) {
        Ok(data) => data,
        Err(e) => {
            error!("failed to read {}: {e}", args.binary.display());
            return ExitCode::FAILURE;
        }
    };

    let config = SocConfig {
        entry_point: args.entry,
        ram_size: args.ram * 1024 * 1024,
        max_instructions: args.max,
        ..SocConfig::default()
    };

    info!(
        binary = %args.binary.display(),
        size = binary.len(),
        entry = format_args!("{:#x}", config.entry_point),
        "loading program"
    );

    let mut soc = match Soc::new(config) {
        Ok(s) => s,
        Err(e) => {
            error!("failed to create soc: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = soc.load_binary(0, &binary) {
        error!("failed to load binary: {e}");
        return ExitCode::FAILURE;
    }

    match soc.run() {
        Ok(()) => {
            info!(instructions = soc.cpu().instruction_count, "emulation completed");
        }
        Err(e) => {
            error!("emulation error: {e}");
            if args.dump {
                soc.dump_state();
            }
            return ExitCode::FAILURE;
        }
    }

    if args.dump {
        soc.dump_state();
    }

    ExitCode::SUCCESS
}
