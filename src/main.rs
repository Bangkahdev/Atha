use chrono::Local;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
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

fn install(a: PackageArgs) -> Result<(), String> {
    require_cmd("pacman")?;
    for p in &a.packages {
        if !valid_pkg(p) {
            return Err(format!("Invalid package name: {p}"));
        }
    }
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
    if a.plan || a.dry_run {
        section(if a.plan {
            "PLAN: Decision Analysis"
        } else {
            "DRY-RUN: Execution Simulation"
        });
        for p in &skip {
            println!("  - {p} -> skip (already installed)");
            record("install", p, "installed", "skipped", "mode");
        }
        for p in &official {
            println!("  - {p} -> install from official");
            record(
                "install",
                p,
                "official",
                "planned",
                if a.plan { "plan" } else { "dry-run" },
            );
        }
        for p in &aur {
            println!("  - {p} -> install from AUR");
            record(
                "install",
                p,
                "aur",
                "planned",
                if a.plan { "plan" } else { "dry-run" },
            );
        }
        info_msg(&format!(
            "Summary: install={} official={} aur={} skip={}",
            official.len() + aur.len(),
            official.len(),
            aur.len(),
            skip.len()
        ));
        success("No package changes applied");
        return Ok(());
    }
    if official.is_empty() && aur.is_empty() {
        warn("Nothing to install");
        return Ok(());
    }
    confirm(a.yes, "Proceed with installation?")?;
    if !official.is_empty() {
        let mut c = Command::new("pacman");
        c.args(["-S", "--needed"]).args(&official);
        if a.yes {
            c.arg("--noconfirm");
        }
        let out = command(c, true)?;
        print_output(&out);
        if !out.status.success() {
            for p in &official {
                record("install", p, "official", "failed", "pacman");
            }
            return Err("Failed to install official packages".into());
        }
        for p in &official {
            record("install", p, "official", "success", "");
        }
    }
    for p in aur {
        require_cmd("git")?;
        require_cmd("makepkg")?;
        let root = env::var_os("ATHA_BUILD_DIR")
            .map(PathBuf::from)
            .or_else(|| dirs::cache_dir().map(|d| d.join("atha/build")))
            .unwrap_or_else(|| PathBuf::from(".atha/build"));
        let dir = root.join(format!("{p}-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let out = Command::new("git")
            .args(["clone", &format!("https://aur.archlinux.org/{p}.git")])
            .arg(&dir)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            record("install", &p, "aur", "failed", "git clone");
            return Err(format!("Failed to clone AUR package: {p}"));
        }
        let out = Command::new("makepkg")
            .args(["-si", "--noconfirm"])
            .current_dir(&dir)
            .output()
            .map_err(|e| e.to_string())?;
        let _ = fs::remove_dir_all(&dir);
        if !out.status.success() {
            record("install", &p, "aur", "failed", "makepkg");
            return Err(format!("Failed to install {p}"));
        }
        record("install", &p, "aur", "success", "");
    }
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
    let mut targets = Vec::new();
    for p in &a.packages {
        if run_quiet("pacman", &["-Qi", p]) {
            targets.push(p.clone());
        } else {
            record("remove", p, "official", "skipped", "not-installed");
        }
    }
    if a.plan || a.dry_run {
        section(if a.plan {
            "PLAN: Decision Analysis"
        } else {
            "DRY-RUN: Execution Simulation"
        });
        for p in &a.packages {
            if targets.contains(p) {
                println!("  - {p} -> remove");
                record("remove", p, "official", "planned", "mode")
            } else {
                println!("  - {p} -> skip (not installed)");
            }
        }
        return Ok(());
    }
    if targets.is_empty() {
        warn("Nothing to remove");
        return Ok(());
    }
    confirm(
        a.yes,
        "Are you sure you want to remove these packages and unneeded dependencies?",
    )?;
    let mut c = Command::new("pacman");
    c.args(["-Rns"]).args(&targets);
    if a.yes {
        c.arg("--noconfirm");
    }
    let out = command(c, true)?;
    print_output(&out);
    if !out.status.success() {
        return Err("Remove failed".into());
    }
    for p in targets {
        record("remove", &p, "official", "success", "");
    }
    success("Package(s) removed successfully");
    Ok(())
}
fn confirm(yes: bool, prompt: &str) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    print!("{prompt} (y/N) ");
    io::stdout().flush().ok();
    let mut s = String::new();
    io::stdin().read_line(&mut s).map_err(|e| e.to_string())?;
    if s.trim().eq_ignore_ascii_case("y") {
        Ok(())
    } else {
        warn("Operation cancelled");
        Err("operation cancelled".into())
    }
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
    let o = Command::new("pacman")
        .args(["-Ss", term])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() && !o.stdout.is_empty() {
        print_output(&o);
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
        return Ok(());
    }
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
    let o = Command::new("pacman")
        .args(["-Si", pkg])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        print_output(&o);
        return Ok(());
    }
    let o = Command::new("pacman")
        .args(["-Qi", pkg])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        warn("Package not found in sync database, showing installed local info:");
        print_output(&o);
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
        return Ok(());
    }
    Err(format!(
        "Package not found in official repositories, local DB, or AUR: {pkg}"
    ))
}

fn update(a: ModeArgs) -> Result<(), String> {
    require_cmd("pacman")?;
    if a.plan || a.dry_run {
        section(if a.plan {
            "PLAN: Decision Analysis"
        } else {
            "DRY-RUN: Execution Simulation"
        });
        let output = if which("checkupdates").is_some() {
            Command::new("checkupdates").output()
        } else {
            warn("'checkupdates' not found; falling back to pacman -Qu");
            Command::new("pacman").args(["-Qu"]).output()
        }
        .map_err(|e| e.to_string())?;
        print_output(&output);
        info_msg(&format!(
            "Would execute: sudo pacman -Syu{}",
            if a.yes { " --noconfirm" } else { "" }
        ));
        record(
            "update",
            "system",
            "official",
            "planned",
            if a.plan { "plan" } else { "dry-run" },
        );
        success("No package changes applied");
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
        return Err("System update failed".into());
    }
    record("update", "system", "official", "success", "");
    success("System updated successfully");
    Ok(())
}

fn list(a: ListArgs) -> Result<(), String> {
    require_cmd("pacman")?;
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
    for line in text.lines().take(a.limit) {
        println!("{line}");
    }
    Ok(())
}
fn doctor() -> Result<(), String> {
    let mut missing = 0;
    for c in ["pacman", "sudo", "git", "makepkg"] {
        if which(c).is_some() {
            success(&format!("{c} OK"))
        } else {
            warn(&format!("{c} missing"));
            missing += 1
        }
    }
    if Path::new("/var/lib/pacman/db.lck").exists() {
        warn("pacman database lock exists")
    } else {
        success("pacman database lock not present")
    }
    for p in [
        state_dir(),
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("atha"),
    ] {
        if fs::create_dir_all(&p).is_ok() {
            success(&format!("Directory writable: {}", p.display()))
        } else {
            warn(&format!("Directory not writable: {}", p.display()))
        }
    }
    if missing > 0 {
        Err(format!("Doctor found {missing} missing dependencies"))
    } else {
        success("Doctor check completed");
        Ok(())
    }
}
fn history(a: HistoryArgs) -> Result<(), String> {
    let text = fs::read_to_string(history_path()).unwrap_or_default();
    let mut rows: Vec<Vec<&str>> = text
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
        for r in rows {
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
