use chrono::Local;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    net::ToSocketAddrs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

#[derive(Parser)]
#[command(
    name = "atha",
    version,
    about = "A safety and workflow layer for pacman"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Install(PackageArgs),
    Remove(PackageArgs),
    Search(SearchArgs),
    Update(ModeArgs),
    List(ListArgs),
    Info(PackageArg),
    Doctor,
    History(HistoryArgs),
}
#[derive(Args)]
struct PackageArgs {
    #[arg(long)]
    plan: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    yes: bool,
    #[arg(required = true)]
    packages: Vec<String>,
}
#[derive(Args)]
struct ModeArgs {
    #[arg(long)]
    plan: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    yes: bool,
}
#[derive(Args)]
struct PackageArg {
    package: String,
}
#[derive(Args)]
struct SearchArgs {
    keyword: String,
}
#[derive(Clone, ValueEnum)]
enum ListType {
    Installed,
    Explicit,
    Aur,
    All,
}
impl ListType {
    fn label(&self) -> &'static str {
        match self {
            ListType::Installed => "installed",
            ListType::Explicit => "explicit",
            ListType::Aur => "aur",
            ListType::All => "all",
        }
    }
}
#[derive(Args)]
struct ListArgs {
    #[arg(default_value = "installed")]
    kind: ListType,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}
#[derive(Args)]
struct HistoryArgs {
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long)]
    full: bool,
    #[arg(long)]
    timeline: bool,
    #[arg(long)]
    summary: bool,
    #[arg(long)]
    action: Option<String>,
    #[arg(long)]
    status: Option<String>,
}

fn main() {
    if let Err(e) = run() {
        log(&format!("ERROR: {e}"));
        eprintln!("[atha] error: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            println!("{}", HELP);
            Ok(())
        }
        Some(Commands::Install(a)) => install(a),
        Some(Commands::Remove(a)) => remove(a),
        Some(Commands::Search(a)) => search(&a.keyword),
        Some(Commands::Update(a)) => update(a),
        Some(Commands::List(a)) => list(a),
        Some(Commands::Info(a)) => info(&a.package),
        Some(Commands::Doctor) => doctor(),
        Some(Commands::History(a)) => history(a),
    }
}

const HELP: &str = "atha - Simple Package Manager Wrapper\n\nCommands: install remove search update list info doctor history\nUse 'atha <command> --help' for command options.";
fn color(code: &str, text: &str) -> String {
    if env::var_os("NO_COLOR").is_some() {
        text.into()
    } else {
        format!("\x1b[{code}m{text}\x1b[0m")
    }
}
fn info_msg(s: &str) {
    println!("{}", color("1;34", &format!("[atha] {s}")));
}
fn warn(s: &str) {
    println!("{}", color("1;33", &format!("[atha] warning: {s}")));
}
fn success(s: &str) {
    println!("{}", color("1;32", &format!("[atha] {s}")));
}
fn section(s: &str) {
    println!("\n{}", color("1;34", &format!("==> {s}")));
}
fn valid_pkg(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-._+@".contains(c))
}
fn require_cmd(name: &str) -> Result<(), String> {
    which(name)
        .ok_or_else(|| format!("Command not found: {name}"))
        .map(|_| ())
}
fn which(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|p| {
        env::split_paths(&p)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    })
}
fn state_dir() -> PathBuf {
    env::var_os("ATHA_STATE_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("XDG_STATE_HOME").map(|p| PathBuf::from(p).join("atha")))
        .or_else(|| dirs::home_dir().map(|p| p.join(".local/state/atha")))
        .unwrap_or_else(|| PathBuf::from(".atha"))
}
fn history_path() -> PathBuf {
    state_dir().join("history.log")
}
fn log_path() -> PathBuf {
    env::var_os("ATHA_LOG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/atha.log"))
}
fn log(msg: &str) {
    let path = log_path();
    let target = if OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .is_ok()
    {
        path
    } else {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("atha/atha.log")
    };
    if let Some(p) = target.parent() {
        let _ = fs::create_dir_all(p);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(target) {
        let _ = writeln!(f, "[{}] {msg}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}
fn record(action: &str, target: &str, source: &str, status: &str, detail: &str) {
    // Free-form, human-readable trace alongside the structured history file.
    log(&format!(
        "{action} target={target} source={source} status={status}{}",
        if detail.is_empty() {
            String::new()
        } else {
            format!(" detail={detail}")
        }
    ));

    let p = history_path();
    if let Some(d) = p.parent() {
        let _ = fs::create_dir_all(d);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(
            f,
            "{}|{action}|{target}|{source}|{status}|{detail}",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );
    }
}
/// Human-readable byte formatting, matching the shell version's `format_bytes()`
/// (B / KiB / MiB / GiB / TiB, whole numbers only for B).
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut val = bytes as f64;
    let mut i = 0usize;
    while val >= 1024.0 && i < UNITS.len() - 1 {
        val /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", val as u64, UNITS[i])
    } else {
        format!("{:.2} {}", val, UNITS[i])
    }
}
fn command(mut cmd: Command, sudo: bool) -> Result<std::process::Output, String> {
    if sudo && nix_uid_nonroot() {
        let mut c = Command::new("sudo");
        c.arg("--").arg(cmd.get_program());
        c.args(cmd.get_args());
        cmd = c;
    }
    cmd.output().map_err(|e| e.to_string())
}
fn nix_uid_nonroot() -> bool {
    unsafe { libc_geteuid() != 0 }
}
#[cfg(unix)]
unsafe fn libc_geteuid() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }
    geteuid()
}
#[cfg(not(unix))]
unsafe fn libc_geteuid() -> u32 {
    1
}

/// RAII guard that removes an AUR build directory when it goes out of scope —
/// including on early `return`/`?` — so a temp dir never lingers after a
/// failed clone/build. Mirrors the shell version's `trap cleanup_on_exit EXIT`.
struct BuildDirGuard(PathBuf);
impl Drop for BuildDirGuard {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

/// Best-effort, non-destructive writability check: if `path` doesn't exist
/// yet, check its parent instead (matching the shell doctor's behavior),
/// then try creating+removing a throwaway file rather than mkdir-ing the
/// real target as a side effect of just checking.
fn path_writable(path: &Path) -> bool {
    let check_target: PathBuf = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    };
    let probe = check_target.join(format!(".atha_write_test_{}", std::process::id()));
    let ok = fs::File::create(&probe).is_ok();
    if ok {
        let _ = fs::remove_file(&probe);
    }
    ok
}

/// DNS reachability check with a short timeout, matching the shell doctor's
/// `timeout 2 getent hosts <host>`.
fn check_host(host: &str) -> bool {
    let (tx, rx) = mpsc::channel();
    let host = host.to_string();
    std::thread::spawn(move || {
        let ok = format!("{host}:80")
            .to_socket_addrs()
            .map(|mut a| a.next().is_some())
            .unwrap_or(false);
        let _ = tx.send(ok);
    });
    rx.recv_timeout(Duration::from_secs(2)).unwrap_or(false)
}

/// Prompts for confirmation unless `yes` is set. Returns `Ok(true)` to
/// proceed, `Ok(false)` if the user declined — declining is a normal
/// outcome, not an error, matching the shell version's `exit 0` on "n".
/// `Err` is reserved for actual I/O failure reading stdin.
fn confirm(yes: bool, prompt: &str) -> Result<bool, String> {
    if yes {
        return Ok(true);
    }
    print!("{prompt} (y/N) ");
    io::stdout().flush().ok();
    let mut s = String::new();
    io::stdin().read_line(&mut s).map_err(|e| e.to_string())?;
    Ok(s.trim().eq_ignore_ascii_case("y"))
}

/// Runs `pacman -S --print --print-format '%n|%s'` to preview the real
/// install transaction (requested packages + pulled-in dependencies) with
/// per-package download size, matching the shell `--plan` simulation.
fn simulate_install_transaction(official: &[String]) -> Option<Vec<(String, u64)>> {
    if official.is_empty() {
        return None;
    }
    let mut c = Command::new("pacman");
    c.args(["-S", "--print", "--print-format", "%n|%s"])
        .args(official);
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut rows = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(2, '|');
        if let (Some(name), Some(size)) = (parts.next(), parts.next()) {
            if let Ok(bytes) = size.trim().parse::<u64>() {
                rows.push((name.to_string(), bytes));
            }
        }
    }
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}
/// Same idea for removal: `pacman -Rns --print` previews the packages (plus
/// now-unneeded dependencies) that would actually be removed, with freed size.
fn simulate_remove_transaction(targets: &[String]) -> Option<Vec<(String, u64)>> {
    if targets.is_empty() {
        return None;
    }
    let mut c = Command::new("pacman");
    c.args(["-Rns", "--print", "--print-format", "%n|%s"])
        .args(targets);
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut rows = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(2, '|');
        if let (Some(name), Some(size)) = (parts.next(), parts.next()) {
            if let Ok(bytes) = size.trim().parse::<u64>() {
                rows.push((name.to_string(), bytes));
            }
        }
    }
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}
fn aur_reachable(pkg: &str) -> bool {
    Command::new("git")
        .args([
            "ls-remote",
            "--exit-code",
            &format!("https://aur.archlinux.org/{pkg}.git"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn install(a: PackageArgs) -> Result<(), String> {
    require_cmd("pacman")?;
    for p in &a.packages {
        if !valid_pkg(p) {
            return Err(format!("Invalid package name: {p}"));
        }
    }
    log(&format!(
        "Install requested for packages: {}",
        a.packages.join(" ")
    ));

    let mut official = Vec::new();
    let mut aur = Vec::new();
    let mut skip = Vec::new();
    for p in &a.packages {
        if run_quiet("pacman", &["-Qi", p]) {
            skip.push(p.clone());
        } else if run_quiet("pacman", &["-Si", p]) {
            official.push(p.clone());
        } else {
            aur.push(p.clone());
        }
    }

    // --- --plan: deep simulation (transaction preview + AUR reachability) ---
    if a.plan {
        section("PLAN: Decision Analysis");
        for p in &skip {
            println!("  - {p} -> skip");
            println!("    reason: already installed");
        }
        for p in &official {
            println!("  - {p} -> install from official");
            println!("    reason: found in official repositories");
        }
        for p in &aur {
            println!("  - {p} -> install from AUR");
            println!("    reason: not found in official repositories, fallback to AUR workflow");
        }

        if !official.is_empty() {
            section("PLAN: Official Transaction Simulation");
            match simulate_install_transaction(&official) {
                Some(txn) => {
                    let total_bytes: u64 = txn.iter().map(|(_, s)| s).sum();
                    info_msg(&format!(
                        "Packages in transaction (requested + dependencies): {}",
                        txn.len()
                    ));
                    info_msg(&format!(
                        "Estimated download size: {}",
                        format_bytes(total_bytes)
                    ));
                    for (name, size) in &txn {
                        let marker = if official.contains(name) {
                            "requested"
                        } else {
                            "dependency"
                        };
                        println!("  - {name} ({}, {marker})", format_bytes(*size));
                    }
                }
                None => warn("Unable to simulate official transaction"),
            }
        }

        if !aur.is_empty() {
            section("PLAN: AUR Reachability");
            for p in &aur {
                if aur_reachable(p) {
                    success(&format!("{p} repository reachable"));
                } else {
                    warn(&format!("{p} repository not reachable or git unavailable"));
                }
            }
        }

        section("PLAN: Summary");
        info_msg(&format!(
            "install={} official={} aur={} skip={}",
            official.len() + aur.len(),
            official.len(),
            aur.len(),
            skip.len()
        ));

        for p in &skip {
            record(
                "install",
                p,
                "installed",
                "skipped",
                "plan:already installed",
            );
        }
        for p in &official {
            record(
                "install",
                p,
                "official",
                "planned",
                "plan:found in official repositories",
            );
        }
        for p in &aur {
            record(
                "install",
                p,
                "aur",
                "planned",
                "plan:not found in official repositories",
            );
        }
        println!();
        success("Plan completed (no changes applied)");
        return Ok(());
    }

    // --- --dry-run: shallow simulation (just the commands that would run) ---
    if a.dry_run {
        log(&format!(
            "Install dry-run requested for packages: {}",
            a.packages.join(" ")
        ));
        section("DRY-RUN: Execution Simulation");
        info_msg("No package changes will be applied");

        for p in &skip {
            warn(&format!("{p} -> already installed (skip)"));
            record(
                "install",
                p,
                "installed",
                "skipped",
                "dry-run:already installed",
            );
        }
        for p in &official {
            info_msg(&format!("{p} -> would execute: sudo pacman -S {p}"));
            record(
                "install",
                p,
                "official",
                "planned",
                "dry-run:found in official repositories",
            );
        }
        for p in &aur {
            if a.yes {
                info_msg(&format!(
                    "{p} -> would execute: git clone https://aur.archlinux.org/{p}.git && makepkg -si --noconfirm"
                ));
            } else {
                info_msg(&format!(
                    "{p} -> would execute: git clone https://aur.archlinux.org/{p}.git && makepkg -si"
                ));
            }
            record(
                "install",
                p,
                "aur",
                "planned",
                "dry-run:not found in official repositories",
            );
        }

        println!();
        info_msg(&format!(
            "Execution summary: install={} official={} aur={} skip={}",
            official.len() + aur.len(),
            official.len(),
            aur.len(),
            skip.len()
        ));
        success("Dry-run completed (no changes applied)");
        return Ok(());
    }

    // --- real execution ---
    section("Execution Plan");
    if !official.is_empty() {
        info_msg(&format!("Official repo targets: {}", official.join(" ")));
    }
    if !aur.is_empty() {
        info_msg(&format!("AUR targets: {}", aur.join(" ")));
    }
    if !skip.is_empty() {
        warn(&format!("Skipped (already installed): {}", skip.join(" ")));
    }
    println!();
    let actionable = official.len() + aur.len();
    info_msg(&format!(
        "Summary: install={actionable} official={} aur={} skip={}",
        official.len(),
        aur.len(),
        skip.len()
    ));

    if actionable == 0 {
        warn("Nothing to install");
        return Ok(());
    }

    if !confirm(a.yes, "Proceed with installation?")? {
        info_msg("Operation cancelled");
        log(&format!(
            "Install cancelled by user for packages: {}",
            a.packages.join(" ")
        ));
        for p in &official {
            record("install", p, "official", "cancelled", "user-cancel");
        }
        for p in &aur {
            record("install", p, "aur", "cancelled", "user-cancel");
        }
        return Ok(());
    }

    for p in &skip {
        log(&format!("Skip install (already installed): {p}"));
        record("install", p, "installed", "skipped", "already-installed");
    }

    if !official.is_empty() {
        info_msg(&format!(
            "Installing official packages: {}",
            official.join(" ")
        ));
        log(&format!(
            "Executing batch install for official packages: {}",
            official.join(" ")
        ));
        let mut c = Command::new("pacman");
        c.args(["-S", "--needed"]).args(&official);
        if a.yes {
            c.arg("--noconfirm");
        }
        let out = command(c, true)?;
        print_output(&out);
        if !out.status.success() {
            for p in &official {
                record("install", p, "official", "failed", "pacman batch failure");
            }
            return Err("Failed to install official packages".into());
        }
        for p in &official {
            success(&format!("{p} installed"));
            log(&format!("Install success: {p}"));
            record("install", p, "official", "success", "");
        }
    }

    if !aur.is_empty() {
        require_cmd("git")?;
        require_cmd("makepkg")?;
        for p in aur {
            info_msg(&format!("Installing {p} from AUR"));
            log(&format!("Source detected for {p}: aur"));

            let root = env::var_os("ATHA_BUILD_DIR")
                .map(PathBuf::from)
                .or_else(|| dirs::cache_dir().map(|d| d.join("atha/build")))
                .unwrap_or_else(|| PathBuf::from(".atha/build"));
            let dir = root.join(format!("{p}-{}", std::process::id()));
            let _ = fs::create_dir_all(&root);
            log(&format!("AUR build directory: {}", dir.display()));
            let _guard = BuildDirGuard(dir.clone());

            let out = Command::new("git")
                .args(["clone", &format!("https://aur.archlinux.org/{p}.git")])
                .arg(&dir)
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                record("install", &p, "aur", "failed", "git clone failed");
                return Err(format!("Failed to clone AUR package: {p}"));
            }
            let mut mk = Command::new("makepkg");
            mk.arg("-si");
            if a.yes {
                mk.arg("--noconfirm");
            }
            let out = mk.current_dir(&dir).output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                record("install", &p, "aur", "failed", "makepkg");
                return Err(format!("Failed install {p}"));
            }
            success(&format!("{p} installed"));
            log(&format!("Install success: {p}"));
            record("install", &p, "aur", "success", "");
            // `_guard` drops here at the end of the loop body, removing `dir`
            // whether we got here normally or bailed out early via `?` above.
        }
    }

    println!();
    success("Install process completed");
    Ok(())
}

fn remove(a: PackageArgs) -> Result<(), String> {
    require_cmd("pacman")?;
    for p in &a.packages {
        if !valid_pkg(p) {
            return Err(format!("Invalid package name: {p}"));
        }
    }
    log(&format!(
        "Remove requested for packages: {}",
        a.packages.join(" ")
    ));

    let mut targets = Vec::new();
    let mut missing = Vec::new();
    for p in &a.packages {
        if run_quiet("pacman", &["-Qi", p]) {
            targets.push(p.clone());
        } else {
            missing.push(p.clone());
        }
    }

    // --- --plan: deep simulation (transaction preview) ---
    if a.plan {
        section("PLAN: Decision Analysis");
        for p in &targets {
            println!("  - {p} -> remove");
            println!("    reason: installed package");
        }
        for p in &missing {
            println!("  - {p} -> skip");
            println!("    reason: not installed");
        }

        if !targets.is_empty() {
            section("PLAN: Transaction Impact");
            match simulate_remove_transaction(&targets) {
                Some(txn) => {
                    let total: u64 = txn.iter().map(|(_, s)| s).sum();
                    info_msg("Transaction impact (remove + unneeded dependencies):");
                    for (name, size) in &txn {
                        println!("  - {name} ({})", format_bytes(*size));
                    }
                    info_msg(&format!(
                        "Estimated transaction size impact (freed space): {}",
                        format_bytes(total)
                    ));
                }
                None => {
                    for p in &targets {
                        let out = Command::new("pacman").args(["-Qi", p]).output().ok();
                        let size = out.and_then(|o| {
                            String::from_utf8_lossy(&o.stdout)
                                .lines()
                                .find(|l| l.starts_with("Installed Size"))
                                .and_then(|l| l.split(':').nth(1).map(|s| s.trim().to_string()))
                        });
                        match size {
                            Some(s) => info_msg(&format!("Estimated freed size for {p}: {s}")),
                            None => warn(&format!(
                                "Unable to simulate remove dependency tree for {p}"
                            )),
                        }
                    }
                }
            }
        }

        println!();
        info_msg(&format!(
            "Summary: remove={} skip={}",
            targets.len(),
            missing.len()
        ));
        for p in &targets {
            record("remove", p, "official", "planned", "plan:installed package");
        }
        for p in &missing {
            record("remove", p, "official", "skipped", "plan:not installed");
        }
        success("Plan completed (no changes applied)");
        return Ok(());
    }

    // --- --dry-run: shallow simulation ---
    if a.dry_run {
        section("DRY-RUN: Execution Simulation");
        info_msg("No package changes will be applied");
        if !targets.is_empty() {
            info_msg(&format!(
                "Would execute: sudo pacman -Rns {}",
                targets.join(" ")
            ));
        }
        for p in &missing {
            warn(&format!("{p} -> already absent (skip)"));
            record("remove", p, "official", "skipped", "dry-run:not installed");
        }
        for p in &targets {
            info_msg(&format!("{p} -> would execute: sudo pacman -Rns {p}"));
            record(
                "remove",
                p,
                "official",
                "planned",
                "dry-run:installed package",
            );
        }
        println!();
        info_msg(&format!(
            "Execution summary: remove={} skip={}",
            targets.len(),
            missing.len()
        ));
        success("Dry-run completed (no changes applied)");
        return Ok(());
    }

    // --- real execution ---
    if targets.is_empty() {
        section("Execution Plan");
        for p in &missing {
            record("remove", p, "official", "skipped", "not-installed");
        }
        info_msg(&format!("Summary: remove=0 skip={}", missing.len()));
        warn("Nothing to remove");
        return Ok(());
    }

    section("Execution Plan");
    info_msg(&format!("Targets: {}", targets.join(" ")));
    info_msg(&format!(
        "Summary: remove={} skip={}",
        targets.len(),
        missing.len()
    ));
    for p in &missing {
        record("remove", p, "official", "skipped", "not-installed");
    }

    if !confirm(
        a.yes,
        "Are you sure you want to remove these packages and unneeded dependencies?",
    )? {
        for p in &targets {
            record("remove", p, "official", "cancelled", "user-cancel");
        }
        log(&format!(
            "Remove cancelled by user for packages: {}",
            a.packages.join(" ")
        ));
        info_msg("Operation cancelled");
        return Ok(());
    }

    info_msg(&format!("Removing: {}", targets.join(" ")));
    log(&format!(
        "Remove requested for packages: {}",
        targets.join(" ")
    ));
    println!();

    let mut c = Command::new("pacman");
    c.args(["-Rns"]).args(&targets);
    if a.yes {
        c.arg("--noconfirm");
    }
    let out = command(c, true)?;
    print_output(&out);
    if !out.status.success() {
        for p in &targets {
            record("remove", p, "official", "failed", "pacman-remove");
        }
        log(&format!(
            "Remove failed for packages: {}",
            targets.join(" ")
        ));
        return Err("Remove failed".into());
    }
    for p in targets {
        record("remove", &p, "official", "success", "");
    }
    log("Remove success");
    println!();
    success("Package(s) removed successfully");
    Ok(())
}
fn run_quiet(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
fn print_output(o: &std::process::Output) {
    let _ = io::stdout().write_all(&o.stdout);
    let _ = io::stderr().write_all(&o.stderr);
}

fn search(term: &str) -> Result<(), String> {
    require_cmd("pacman")?;
    if term.is_empty() {
        return Err("Package name cannot be empty".into());
    }
    log(&format!("Search requested with term: {term}"));
    let o = Command::new("pacman")
        .args(["-Ss", term])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() && !o.stdout.is_empty() {
        print_output(&o);
        log(&format!("Search success in official repo for term: {term}"));
        return Ok(());
    }
    let url = format!("https://aur.archlinux.org/rpc/v5/search/{term}");
    let data: AurSearch = reqwest::blocking::get(url)
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    if data.resultcount > 0 {
        section(&format!("AUR Results ({})", data.resultcount));
        for p in data.results.iter().take(15) {
            println!(
                "aur/{} {}\n    {}",
                p.name,
                p.version,
                p.description.as_deref().unwrap_or("No description")
            );
        }
        log(&format!("Search success in AUR for term: {term}"));
        return Ok(());
    }
    log(&format!("Search no results: {term}"));
    Err(format!("No packages found for: {term} (official or AUR)"))
}
#[derive(Deserialize)]
struct AurSearch {
    resultcount: usize,
    results: Vec<AurPkg>,
}
#[derive(Deserialize)]
struct AurPkg {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "URL")]
    _url: Option<String>,
    #[serde(rename = "Maintainer")]
    _maintainer: Option<String>,
}
fn info(pkg: &str) -> Result<(), String> {
    if !valid_pkg(pkg) {
        return Err(format!("Invalid package name: {pkg}"));
    }
    require_cmd("pacman")?;
    log(&format!("Info requested for package: {pkg}"));
    let o = Command::new("pacman")
        .args(["-Si", pkg])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        print_output(&o);
        log(&format!("Info success for official package: {pkg}"));
        return Ok(());
    }
    let o = Command::new("pacman")
        .args(["-Qi", pkg])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        warn("Package not found in sync database, showing installed local info:");
        print_output(&o);
        log(&format!("Info success for local package: {pkg}"));
        return Ok(());
    }
    let d: serde_json::Value =
        reqwest::blocking::get(format!("https://aur.archlinux.org/rpc/v5/info/{pkg}"))
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;
    if d["resultcount"].as_u64() == Some(1) {
        let p = &d["results"][0];
        println!(
            "Name: {}\nVersion: {}\nDescription: {}\nURL: {}\nMaintainer: {}",
            p["Name"], p["Version"], p["Description"], p["URL"], p["Maintainer"]
        );
        log(&format!("Info success for AUR package: {pkg}"));
        return Ok(());
    }
    log(&format!("Info failed for package: {pkg}"));
    Err(format!(
        "Package not found in official repositories, local DB, or AUR: {pkg}"
    ))
}

fn count_foreign_packages() -> usize {
    Command::new("pacman")
        .args(["-Qm"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count()
        })
        .unwrap_or(0)
}
fn get_pending_updates() -> (String, usize) {
    let out = if which("checkupdates").is_some() {
        Command::new("checkupdates").output()
    } else {
        warn("'checkupdates' not found (install pacman-contrib). Falling back to local db check (pacman -Qu).");
        Command::new("pacman").args(["-Qu"]).output()
    };
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
        .unwrap_or_default();
    let count = text.lines().filter(|l| !l.trim().is_empty()).count();
    (text, count)
}

fn update(a: ModeArgs) -> Result<(), String> {
    require_cmd("pacman")?;
    info_msg("Updating system");
    log("System update requested");
    println!();

    let foreign_count = count_foreign_packages();
    if foreign_count > 0 {
        info_msg(&format!(
            "Note: You have {foreign_count} AUR/local package(s) installed. These must be updated separately."
        ));
    }

    if a.plan || a.dry_run {
        section(if a.plan {
            "PLAN: Decision Analysis"
        } else {
            "DRY-RUN: Execution Simulation"
        });
        info_msg("Checking available updates");
        let (updates_output, count) = get_pending_updates();

        if !updates_output.is_empty() {
            if a.plan {
                for line in updates_output.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let (Some(pkg), Some(old), Some(new)) =
                        (parts.first(), parts.get(1), parts.get(3))
                    {
                        println!("  - {pkg} -> update");
                        println!("    reason: update available ({old} -> {new})");
                    }
                }
            } else {
                println!("{updates_output}");
            }
            info_msg(&format!("Pending updates: {count}"));
        } else if a.plan {
            info_msg("Decision: no update required");
        } else {
            info_msg("No pending updates detected");
        }

        let mut run_cmd = String::from("sudo pacman -Syu");
        if a.yes {
            run_cmd.push_str(" --noconfirm");
        }
        if !nix_uid_nonroot() {
            run_cmd = run_cmd.replacen("sudo ", "", 1);
        }
        info_msg(&format!("Would execute: {run_cmd}"));

        let mode_label = if a.plan { "plan" } else { "dry-run" };
        record(
            "update",
            "system",
            "official",
            "planned",
            &format!("{mode_label} count={count}"),
        );
        success(if a.plan {
            "Plan completed (no changes applied)"
        } else {
            "Dry-run completed (no changes applied)"
        });
        return Ok(());
    }

    let mut c = Command::new("pacman");
    c.args(["-Syu"]);
    if a.yes {
        c.arg("--noconfirm");
    }
    let o = command(c, true)?;
    print_output(&o);
    if !o.status.success() {
        record("update", "system", "official", "failed", "pacman-syu");
        log("System update failed");
        return Err("System update failed".into());
    }
    record("update", "system", "official", "success", "");
    log("System update success");
    success("System updated successfully");
    Ok(())
}

fn list(a: ListArgs) -> Result<(), String> {
    require_cmd("pacman")?;
    log(&format!(
        "List requested: {} (limit={})",
        a.kind.label(),
        a.limit
    ));
    let args = match a.kind {
        ListType::Installed => vec!["-Q"],
        ListType::Explicit => vec!["-Qe"],
        ListType::Aur => vec!["-Qm"],
        ListType::All => vec!["-Sl"],
    };
    let o = Command::new("pacman")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err("pacman list failed".into());
    }
    let text = String::from_utf8_lossy(&o.stdout);
    match a.kind {
        // Only "all" (repository listing) is paginated with --limit, matching
        // the shell version — "installed"/"explicit"/"aur" always show in full.
        ListType::All => {
            for line in text.lines().take(a.limit) {
                println!("{line}");
            }
            println!();
            info_msg(&format!(
                "Showing first {} packages (use 'atha list all --limit N' for more)",
                a.limit
            ));
        }
        _ => {
            for line in text.lines() {
                println!("{line}");
            }
        }
    }
    Ok(())
}
fn doctor() -> Result<(), String> {
    log("Doctor check requested");
    let mut missing = 0;
    let mut warnings = 0;

    section("Core tools check");
    for c in ["pacman", "sudo", "git", "makepkg"] {
        if which(c).is_some() {
            success(&format!("{c} OK"));
            log(&format!("Doctor: {c} OK"));
        } else {
            print!("{}", ""); // keep formatting consistent with warn()
            warn(&format!("{c} missing"));
            log(&format!("Doctor: {c} missing"));
            missing += 1;
        }
    }

    println!();
    section("Runtime checks");
    if Path::new("/var/lib/pacman/db.lck").exists() {
        warn("pacman database lock exists (/var/lib/pacman/db.lck)");
        warnings += 1;
    } else {
        success("pacman database lock not present");
    }

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("atha");
    if path_writable(&cache_dir) {
        success(&format!(
            "Cache directory writable: {}",
            cache_dir.display()
        ));
    } else {
        warn(&format!(
            "Cache directory not writable: {}",
            cache_dir.display()
        ));
        warnings += 1;
    }
    let state = state_dir();
    if path_writable(&state) {
        success(&format!("State directory writable: {}", state.display()));
    } else {
        warn(&format!(
            "State directory not writable: {}",
            state.display()
        ));
        warnings += 1;
    }

    println!();
    section("Connectivity checks");
    for host in ["archlinux.org", "aur.archlinux.org"] {
        if check_host(host) {
            success(&format!("DNS reachable: {host}"));
        } else {
            warn(&format!("DNS unresolved: {host}"));
            warnings += 1;
        }
    }

    println!();
    if missing == 0 {
        if warnings == 0 {
            success("Doctor check completed (healthy)");
        } else {
            warn(&format!("Doctor completed with {warnings} warning(s)"));
        }
        Ok(())
    } else {
        warn(&format!(
            "Doctor found {missing} missing dependency(ies) and {warnings} warning(s)"
        ));
        Err(format!("Doctor found {missing} missing dependencies"))
    }
}
fn history(a: HistoryArgs) -> Result<(), String> {
    log(&format!(
        "History requested (limit={} full={} timeline={} summary={} action={:?} status={:?})",
        a.limit, a.full, a.timeline, a.summary, a.action, a.status
    ));
    let raw = fs::read_to_string(history_path()).unwrap_or_default();
    if raw.trim().is_empty() {
        warn("No history available yet");
        return Ok(());
    }

    let mut rows: Vec<Vec<&str>> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('|').collect::<Vec<&str>>())
        .filter(|r: &Vec<&str>| r.len() >= 6)
        .filter(|r| a.action.as_deref().map_or(true, |x| r[1] == x))
        .filter(|r| a.status.as_deref().map_or(true, |x| r[4] == x))
        .collect();
    if rows.is_empty() {
        warn("No history matched the selected filters");
        return Ok(());
    }
    let start = rows.len().saturating_sub(a.limit);
    rows.drain(0..start);
    if a.summary {
        section("Summary by action");
        summary(&rows, 1);
        section("Summary by status");
        summary(&rows, 4)
    } else {
        for r in &rows {
            if a.timeline {
                println!(
                    "[{}] {:<7} {:<20} status={:<9} source={:<8} detail={}",
                    r[0],
                    r[1].to_uppercase(),
                    r[2],
                    r[4],
                    r[3],
                    if r[5].is_empty() { "-" } else { r[5] }
                );
            } else if a.full {
                println!(
                    "{:<19} | {:<7} | {:<24} | {:<8} | {:<9} | {}",
                    r[0], r[1], r[2], r[3], r[4], r[5]
                );
            } else {
                println!(
                    "{:<19} | {:<7} | {:<24} | {:<8} | {:<9}",
                    r[0], r[1], r[2], r[3], r[4]
                );
            }
        }
    }
    println!();
    success("History shown");
    Ok(())
}
fn summary(rows: &[Vec<&str>], idx: usize) {
    let mut m = std::collections::BTreeMap::new();
    for r in rows {
        *m.entry(r[idx]).or_insert(0) += 1;
    }
    for (k, v) in m {
        println!("  - {k}: {v}")
    }
}
