mod config;
use config::Config;
use log::Level;
use regex::Regex;
use std::{
    io::{BufRead, BufReader},
    process::{self, exit},
    sync::{Arc, OnceLock},
};

use crate::{health, machine};

pub fn run() {
    let config = Config::from_env();
    let group = config.group().to_string();
    let machine_config = Arc::new(config.machine.clone());

    info!(target: "lazymc-docker-proxy::entrypoint", "Starting lazymc for group: {}...", group);

    let mut child: process::Child = config
        .start_command()
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    // Forward child stdout/stderr through our logger
    let mut stdout = child.stdout.take();
    let group_clone = group.clone();
    let mc_clone = Arc::clone(&machine_config);
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout.take().unwrap());
        for line in reader.lines() {
            wrap_log(&group_clone, line, &mc_clone);
        }
    });

    let mut stderr = child.stderr.take();
    let group_clone2 = group.clone();
    let mc_clone2 = Arc::clone(&machine_config);
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr.take().unwrap());
        for line in reader.lines() {
            wrap_log(&group_clone2, line, &mc_clone2);
        }
    });

    // On SIGTERM stop the remote server and shut the PC down
    let mc_shutdown = Arc::clone(&machine_config);
    ctrlc::set_handler(move || {
        info!(target: "lazymc-docker-proxy::entrypoint", "Received exit signal. Stopping server...");
        machine::stop_server(&mc_shutdown);
        machine::shutdown_pc(&mc_shutdown);
        exit(0);
    })
    .unwrap();

    health::healthy();

    loop {
        std::thread::park();
    }
}

/// Re-emit a log line from a child process, tagged with the group name
fn wrap_log(group: &str, line: Result<String, std::io::Error>, machine_config: &Arc<machine::MachineConfig>) {
    static LOG_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = LOG_REGEX.get_or_init(|| {
        Regex::new(r"(?P<level>[A-Z]+)\s+(?P<target>[a-zA-Z0-9:_-]+)\s+>\s+(?P<message>.+)$")
            .unwrap()
    });

    if let Ok(line) = line {
        if let Some(caps) = regex.captures(&line) {
            let level: Level = caps
                .name("level")
                .unwrap()
                .as_str()
                .parse()
                .unwrap_or(Level::Info);
            let target = caps.name("target").unwrap().as_str();
            let message = caps.name("message").unwrap().as_str();

            let wrapped_target = format!("{}::{}", group, target);
            let msg = message.to_string();
            log!(target: &wrapped_target, level, "{}", msg);
            handle_log(group, &level, &msg, machine_config);
        } else {
            print!("{}", line);
        }
    }
}

/// React to specific lazymc log messages that require intervention
fn handle_log(group: &str, level: &Level, message: &str, machine_config: &Arc<machine::MachineConfig>) {
    if let (Level::Warn, "Failed to stop server, no more suitable stopping method to use") =
        (level, message)
    {
        warn!(target: "lazymc-docker-proxy::entrypoint",
            "Unexpected server state for group '{}'. Force-stopping Minecraft and shutting down PC...", group);
        machine::stop_server(machine_config);
        machine::shutdown_pc(machine_config);
        info!(target: "lazymc-docker-proxy::entrypoint", "Force shutdown sequence complete for group '{}'.", group);
    }
}
