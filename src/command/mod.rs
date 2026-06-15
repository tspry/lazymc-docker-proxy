use std::{process, thread, time::Duration};

use crate::machine::{self, MachineConfig};

/// Run as the lazymc server start/stop command for the given group.
/// Powers on the PC, starts Minecraft, then waits for SIGTERM to shut it all down.
pub fn run(group: String) {
    info!(target: "lazymc-docker-proxy::command", "Received start command for group: {}", group);

    let config = MachineConfig::from_env();

    machine::power_on(&config);
    machine::start_server(&config);

    let config_clone = config.clone();
    ctrlc::set_handler(move || {
        info!(target: "lazymc-docker-proxy::command", "Received SIGTERM, stopping server for group: {}", group);
        machine::stop_server(&config_clone);
        machine::shutdown_pc(&config_clone);
        process::exit(0);
    })
    .unwrap();

    loop {
        trace!(target: "lazymc-docker-proxy::command", "Waiting for SIGTERM...");
        thread::sleep(Duration::from_secs(5));
    }
}
