mod app;
mod catalog;
mod grid;
mod telemetry;
mod tres;

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use app::App;
use catalog::{Catalog, WeaponStatsBook};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use telemetry::{
    protocol::{DEFAULT_BIND_ADDRESS, JsonDecoder, TelemetryMessage},
    receiver::{ReceiverEvent, TelemetryReceiver},
};
use tres::{CharacterDocument, validate_equipment_values};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    if let Some((duration, bind_address)) = parse_telemetry_probe_args(&args)? {
        return run_telemetry_probe(duration, &bind_address);
    }
    let spawn_airdrop = args.iter().any(|arg| arg == "--spawn-airdrop");
    let spawn_boss = args
        .iter()
        .any(|arg| arg == "--spawn-boss" || arg == "--spawn-bogeyman");
    let check = args.iter().any(|arg| arg == "--check");
    if spawn_airdrop || spawn_boss {
        bail!(
            "Standalone summon commands are disabled: use Radar in the Toolkit with a compatible live summon bridge"
        );
    }
    let telemetry_bind = parse_telemetry_bind_arg(&args)?;
    let requested = parse_save_arg(&args)?;
    let path = match requested {
        Some(path) => path,
        None => discover_save().ok_or_else(|| {
            anyhow::anyhow!("could not find Character.tres; pass --save <path> (see --help)")
        })?,
    };

    let catalog = Catalog::load()?;
    let document = CharacterDocument::load(&path)?;
    if check {
        return check_document(&document, &catalog);
    }

    enable_raw_mode().context("could not enable terminal raw mode")?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    let mut app = App::new(document, catalog)?;
    app.enable_telemetry(&telemetry_bind);
    app.run(&mut terminal)
}

fn run_telemetry_probe(duration: Duration, bind_address: &str) -> Result<()> {
    let receiver = TelemetryReceiver::bind(bind_address, Arc::new(JsonDecoder))
        .with_context(|| format!("could not listen on UDP {bind_address}"))?;
    println!(
        "Listening for Road to Vostok telemetry on UDP {} for {:.1}s...",
        receiver.local_address(),
        duration.as_secs_f64()
    );
    let deadline = Instant::now() + duration;
    let mut valid_packets = 0_u64;
    let mut malformed_packets = 0_u64;
    while Instant::now() < deadline {
        for event in receiver.try_iter() {
            match event {
                ReceiverEvent::Message {
                    received_at,
                    source,
                    message,
                } => {
                    valid_packets += 1;
                    match message {
                        TelemetryMessage::Snapshot(snapshot) => println!(
                            "snapshot #{valid_packets} from {source} at {received_at:?}: map={} player={} pos=[{:.2}, {:.2}, {:.2}] heading={:.2} ai={} bosses={} loot={}",
                            if snapshot.map.name.is_empty() {
                                "unknown"
                            } else {
                                &snapshot.map.name
                            },
                            snapshot.player.id,
                            snapshot.player.position[0],
                            snapshot.player.position[1],
                            snapshot.player.position[2],
                            snapshot.player.heading,
                            snapshot.ai.len(),
                            snapshot.ai.iter().filter(|entity| entity.boss).count(),
                            snapshot.loot.len()
                        ),
                        TelemetryMessage::Gunshot(shot) => println!(
                            "gunshot #{valid_packets} from {source} at {received_at:?}: shooter={} pos=[{:.2}, {:.2}, {:.2}]",
                            shot.shooter_id, shot.position[0], shot.position[1], shot.position[2]
                        ),
                    }
                }
                ReceiverEvent::Malformed {
                    received_at,
                    source,
                    error,
                } => {
                    malformed_packets += 1;
                    eprintln!("ignored malformed packet from {source} at {received_at:?}: {error}");
                }
                ReceiverEvent::SocketError(error) => {
                    eprintln!("telemetry socket error (receiver remains active): {error}");
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    println!(
        "Telemetry probe finished: {valid_packets} valid packet(s), {malformed_packets} malformed packet(s)."
    );
    if valid_packets == 0 {
        bail!("no telemetry packets arrived")
    }
    Ok(())
}

fn parse_telemetry_probe_args(args: &[String]) -> Result<Option<(Duration, String)>> {
    if !args.iter().any(|argument| argument == "--telemetry-probe") {
        return Ok(None);
    }
    let mut seconds = 30.0_f64;
    let mut bind_address = DEFAULT_BIND_ADDRESS.to_owned();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--telemetry-probe" => {}
            "--duration" => {
                index += 1;
                seconds = args
                    .get(index)
                    .ok_or_else(|| anyhow::anyhow!("--duration requires seconds"))?
                    .parse::<f64>()
                    .context("--duration must be a number of seconds")?;
            }
            "--telemetry-bind" => {
                index += 1;
                bind_address = args
                    .get(index)
                    .ok_or_else(|| anyhow::anyhow!("--telemetry-bind requires an address"))?
                    .clone();
            }
            option => bail!("{option} cannot be combined with --telemetry-probe"),
        }
        index += 1;
    }
    if !seconds.is_finite() || seconds <= 0.0 {
        bail!("--duration must be greater than zero");
    }
    Ok(Some((Duration::from_secs_f64(seconds), bind_address)))
}

pub(crate) fn request_airdrop(save_path: &Path) -> Result<String> {
    request_runtime_command(save_path, "spawn_airdrop")
}

fn request_runtime_command(save_path: &Path, action: &str) -> Result<String> {
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    if !matches!(
        action,
        "spawn_airdrop" | "spawn_boss" | "spawn_punisher" | "spawn_bogeyman"
    ) {
        bail!("unsupported runtime command action");
    }
    let directory = save_path
        .parent()
        .context("Character.tres has no parent directory")?;
    let request_path = directory.join("rtv-toolkit-command.cfg");
    if request_path.exists() {
        bail!(
            "a runtime command is already pending at {}; wait for it to be consumed",
            request_path.display()
        );
    }
    let command_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before the Unix epoch")?
            .as_nanos()
    );
    let temporary_path = directory.join(format!(".rtv-toolkit-command.{}.tmp", std::process::id()));
    let mut temporary = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .with_context(|| format!("could not create {}", temporary_path.display()))?;
    write!(
        temporary,
        "[command]\nid=\"{command_id}\"\naction=\"{action}\"\n"
    )?;
    temporary.sync_all()?;
    if let Err(error) = fs::rename(&temporary_path, &request_path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error).with_context(|| {
            format!(
                "could not publish airdrop request at {}",
                request_path.display()
            )
        });
    }
    Ok(command_id)
}

fn check_document(document: &CharacterDocument, catalog: &Catalog) -> Result<()> {
    let inventory = document.inventory()?;
    let equipment = document.equipment()?;
    let report = document.validation(catalog)?;
    let weapon_stats = WeaponStatsBook::load()?;
    let equipment_errors = validate_equipment_values(&equipment, catalog, &weapon_stats);
    let equipped_count = equipment.iter().filter(|slot| slot.item.is_some()).count();
    println!("Save: {}", document.path().display());
    println!("Inventory items: {}", inventory.len());
    println!(
        "Equipment: {equipped_count}/{} slots populated",
        equipment.len()
    );
    println!(
        "Occupied cells: {}/{}",
        report.occupied_cells,
        grid::COLS * grid::ROWS
    );
    if let Some(world) = document.world_state_result()? {
        println!("World: day {} at {}", world.day, world.clock());
    } else {
        println!("World: not found (optional)");
    }

    let mut errors = report.errors.clone();
    errors.extend(equipment_errors);
    errors.extend(document.vital_validation_errors());
    if errors.is_empty() {
        println!(
            "Validation: OK (character vitals, inventory, equipment, and available world data)"
        );
        Ok(())
    } else {
        println!("Validation: FAILED");
        for error in &errors {
            println!("- {error}");
        }
        bail!("save validation failed with {} error(s)", errors.len())
    }
}

fn parse_telemetry_bind_arg(args: &[String]) -> Result<String> {
    let mut address = DEFAULT_BIND_ADDRESS.to_owned();
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--telemetry-bind" {
            index += 1;
            address = args
                .get(index)
                .ok_or_else(|| anyhow::anyhow!("--telemetry-bind requires an address"))?
                .clone();
        }
        index += 1;
    }
    Ok(address)
}

fn parse_save_arg(args: &[String]) -> Result<Option<PathBuf>> {
    let mut path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--check" | "--spawn-airdrop" | "--spawn-boss" | "--spawn-bogeyman" => {}
            "--telemetry-bind" => {
                index += 1;
                if index >= args.len() {
                    bail!("--telemetry-bind requires an address");
                }
            }
            "--save" | "-s" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow::anyhow!("--save requires a path"))?;
                path = Some(PathBuf::from(value));
            }
            option if option.starts_with('-') => bail!("unknown option: {option}"),
            value => {
                if path.is_some() {
                    bail!("more than one save path was provided");
                }
                path = Some(PathBuf::from(value));
            }
        }
        index += 1;
    }
    Ok(path)
}

fn discover_save() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(appdata) = env::var_os("APPDATA") {
        candidates.push(PathBuf::from(appdata).join("Road to Vostok/Character.tres"));
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        candidates
            .push(PathBuf::from(profile).join("AppData/Roaming/Road to Vostok/Character.tres"));
    }
    candidates.push(PathBuf::from("Character.tres"));

    // WSL: discover Windows profiles instead of assuming the Linux username
    // matches the Windows account name.
    let users = Path::new("/mnt/c/Users");
    if let Ok(entries) = fs::read_dir(users) {
        for entry in entries.flatten() {
            candidates.push(
                entry
                    .path()
                    .join("AppData/Roaming/Road to Vostok/Character.tres"),
            );
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn print_help() {
    println!(
        "rtv-toolkit {version}\n\
         Safe Ratatui inventory editor for Road to Vostok.\n\n\
         USAGE:\n  \
           rtv-toolkit [--save <Character.tres>] [--telemetry-bind <address>]\n  \
           rtv-toolkit --check [--save <Character.tres>]\n  \
           rtv-toolkit --telemetry-probe [--duration <seconds>] [--telemetry-bind <address>]\n\n\
         OPTIONS:\n  \
           -s, --save <path>  Character save to open\n  \
               --check        Validate and print a report without starting the UI\n  \
               --telemetry-probe  Print live UDP telemetry and exit\n  \
               --spawn-airdrop/--spawn-boss/--spawn-bogeyman  Disabled; use Radar with a compatible live summon bridge\n  \
               --duration <s>     Probe duration in seconds (default: 30)\n  \
               --telemetry-bind <address>  UDP listen address (default: {telemetry})\n  \
           -h, --help         Show this help\n\n\
         With no path, the application checks APPDATA, USERPROFILE, the current\n\
         directory, and Windows user profiles mounted under /mnt/c/Users.",
        version = env!("CARGO_PKG_VERSION"),
        telemetry = DEFAULT_BIND_ADDRESS
    );
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

#[cfg(test)]
mod command_tests {
    use super::{request_airdrop, request_runtime_command};
    use std::{fs, path::PathBuf};

    #[test]
    fn writes_atomic_narrowly_scoped_runtime_requests() {
        let directory =
            std::env::temp_dir().join(format!("rtv-toolkit-airdrop-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let save = directory.join("Character.tres");
        let command_id = request_airdrop(&save).unwrap();
        assert!(!command_id.is_empty());
        let request_path: PathBuf = directory.join("rtv-toolkit-command.cfg");
        let request = fs::read_to_string(&request_path).unwrap();
        assert!(request.contains("action=\"spawn_airdrop\""));
        assert!(request.contains("id=\""));
        assert!(request_airdrop(&save).is_err());

        fs::remove_file(&request_path).unwrap();
        request_runtime_command(&save, "spawn_punisher").unwrap();
        let request = fs::read_to_string(&request_path).unwrap();
        assert!(request.contains("action=\"spawn_punisher\""));
        fs::remove_file(&request_path).unwrap();
        request_runtime_command(&save, "spawn_bogeyman").unwrap();
        let request = fs::read_to_string(&request_path).unwrap();
        assert!(request.contains("action=\"spawn_bogeyman\""));
        assert!(request_runtime_command(&save, "arbitrary_eval").is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
