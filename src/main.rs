#[macro_use]
extern crate log;

mod command;
mod entrypoint;
mod health;
mod logging;
mod machine;

use clap::Parser;

/// Proxy that puts a remote Minecraft PC to sleep when idle and wakes it on player join
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Execute with this flag when running as the lazymc server start command
    #[arg(short, long)]
    command: bool,

    /// The lazymc group name
    #[arg(short, long, requires_if("command", "true"))]
    group: Option<String>,

    /// Execute with this flag when running as a health check
    #[arg(short, long)]
    health: bool,
}

fn main() {
    logging::init();

    let args: Args = Args::parse();

    if args.command {
        command::run(args.group.unwrap());
    } else if args.health {
        health::run();
    } else {
        entrypoint::run();
    }
}
