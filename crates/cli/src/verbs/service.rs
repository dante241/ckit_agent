// `ckit service` — run the Board dashboard (`ckit harness web`) as a per-user
// background service: started at login, restarted if it dies, logs to
// `~/.cache/ckit/board-service.log`. macOS: a LaunchAgent; Linux: a systemd
// user unit. Board runs live in their own process group (and, on Linux, the
// unit uses `KillMode=process`), so `restart` (e.g. after `ckit up`) does not
// cut a running step: the new dashboard adopts it.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::Args as ClapArgs;

use crate::ui;

const DEFAULT_PORT: u16 = 8731;

#[derive(ClapArgs, Debug)]
#[command(
    after_help = indoc::indoc! {"
        EXAMPLES
          ckit service                 status (installed? running? pid, port, log)
          ckit service install         start the Board at login, now and on every login
          ckit service install --port 8799
          ckit service restart         restart (running Board steps keep going)
          ckit service logs            last lines of the service log
          ckit service uninstall       stop it and remove it
    "}
)]
pub struct Args {
    /// status (default) | install | uninstall | restart | logs
    pub action: Option<String>,
    /// `install`: dashboard port (default 8731)
    #[arg(long)]
    pub port: Option<u16>,
}

pub fn run(a: Args) -> Result<()> {
    let home = dirs::home_dir().context("no home dir")?;
    let svc = Service::new(&home);
    match a.action.as_deref().unwrap_or("status") {
        "status" => status(&svc),
        "install" => install(&svc, a.port.unwrap_or(DEFAULT_PORT)),
        "uninstall" | "remove" => uninstall(&svc),
        "restart" => {
            if !svc.installed() {
                bail!("chưa cài service — chạy `ckit service install`");
            }
            svc.restart()?;
            ui::ok("đã restart service Board (các bước đang chạy vẫn tiếp tục)");
            Ok(())
        }
        "logs" | "log" => logs(&svc),
        other => bail!("không hiểu `{other}` — dùng status | install | uninstall | restart | logs"),
    }
}

/// After `ckit up` swapped the binary: restart an installed service so it runs
/// the new one. Silent no-op when no service is installed.
pub fn restart_if_installed() {
    let Some(home) = dirs::home_dir() else { return };
    let svc = Service::new(&home);
    if svc.installed() {
        match svc.restart() {
            Ok(()) => ui::ok("service Board đã chạy bản mới"),
            Err(e) => ui::warn(&format!("không restart được service Board: {e}")),
        }
    }
}

struct Service {
    home: PathBuf,
}

const LABEL: &str = "ckit.board";
const UNIT: &str = "ckit-board.service";

impl Service {
    fn new(home: &Path) -> Self {
        Self { home: home.to_path_buf() }
    }

    fn file(&self) -> PathBuf {
        if cfg!(target_os = "macos") {
            self.home.join("Library/LaunchAgents").join(format!("{LABEL}.plist"))
        } else {
            self.home.join(".config/systemd/user").join(UNIT)
        }
    }

    fn log(&self) -> PathBuf {
        self.home.join(".cache/ckit/board-service.log")
    }

    fn installed(&self) -> bool {
        self.file().exists()
    }

    fn domain() -> Result<String> {
        let uid = cmd_out("id", &["-u"]).context("id -u")?;
        Ok(format!("gui/{}", uid.trim()))
    }

    /// Load the written file and start it.
    fn load(&self) -> Result<()> {
        if cfg!(target_os = "macos") {
            let d = Self::domain()?;
            // A stale registration (older file) makes bootstrap fail: drop it first.
            let _ = Command::new("launchctl").args(["bootout", &format!("{d}/{LABEL}")]).output();
            run_cmd("launchctl", &["bootstrap", &d, &self.file().display().to_string()])
        } else {
            run_cmd("systemctl", &["--user", "daemon-reload"])?;
            run_cmd("systemctl", &["--user", "enable", "--now", UNIT])
        }
    }

    fn unload(&self) -> Result<()> {
        if cfg!(target_os = "macos") {
            let _ = Command::new("launchctl").args(["bootout", &format!("{}/{LABEL}", Self::domain()?)]).output();
        } else {
            let _ = Command::new("systemctl").args(["--user", "disable", "--now", UNIT]).output();
            let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
        }
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        if cfg!(target_os = "macos") {
            run_cmd("launchctl", &["kickstart", "-k", &format!("{}/{LABEL}", Self::domain()?)])
        } else {
            run_cmd("systemctl", &["--user", "restart", UNIT])
        }
    }

    /// pid of the running service, None when stopped.
    fn pid(&self) -> Option<u32> {
        let out = if cfg!(target_os = "macos") {
            cmd_out("launchctl", &["print", &format!("{}/{LABEL}", Self::domain().ok()?)])?
        } else {
            cmd_out("systemctl", &["--user", "show", "-p", "MainPID", "--value", UNIT])?
        };
        parse_pid(&out)
    }
}

/// `launchctl print` (`pid = 123`) or `systemctl show --value` (`123`, 0 = stopped).
fn parse_pid(out: &str) -> Option<u32> {
    let v = out.lines().find_map(|l| l.trim().strip_prefix("pid = ")).unwrap_or(out.trim());
    v.trim().parse().ok().filter(|p| *p != 0)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// LaunchAgent: run the dashboard at login, keep it alive, log to `log`. `PATH`
/// is the installer's (omp, git, curl, node live in user dirs launchd does not know).
fn plist(bin: &Path, port: u16, home: &Path, path_env: &str, log: &Path) -> String {
    let e = |s: &str| xml_escape(s);
    let args = [bin.display().to_string(), "harness".into(), "web".into(), "--no-open".into(), "--port".into(), port.to_string()];
    let args: String = args.iter().map(|a| format!("    <string>{}</string>\n", e(a))).collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
{args}  </array>
  <key>WorkingDirectory</key>
  <string>{home}</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>{home}</string>
    <key>PATH</key>
    <string>{path}</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ThrottleInterval</key>
  <integer>10</integer>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#,
        home = e(&home.display().to_string()),
        path = e(path_env),
        log = e(&log.display().to_string()),
    )
}

/// systemd user unit. `KillMode=process`: stopping/restarting kills only the
/// dashboard, not the Board runs it started (the next dashboard adopts them).
fn unit(bin: &Path, port: u16, path_env: &str, log: &Path) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    format!(
        "[Unit]\nDescription=ckit Board (dashboard + task runner)\n\n[Service]\nExecStart={} harness web --no-open --port {port}\nEnvironment={}\nRestart=always\nRestartSec=10\nKillMode=process\nStandardOutput=append:{}\nStandardError=append:{}\n\n[Install]\nWantedBy=default.target\n",
        q(&bin.display().to_string()),
        q(&format!("PATH={path_env}")),
        log.display(),
        log.display(),
    )
}

fn port_busy(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), std::time::Duration::from_millis(300)).is_ok()
}

fn install(svc: &Service, port: u16) -> Result<()> {
    ui::header("ckit service install");
    let bin = std::env::current_exe().context("current_exe")?;
    let bin = std::fs::canonicalize(&bin).unwrap_or(bin);
    if bin.components().any(|c| c.as_os_str() == "target") {
        ui::warn(&format!("đang cài từ bản build tạm {} — service sẽ chạy đúng file này; nên cài từ ~/.local/bin/ckit", bin.display()));
    }
    let path_env = std::env::var("PATH").unwrap_or_default();
    let log = svc.log();
    if let Some(d) = log.parent() {
        std::fs::create_dir_all(d)?;
    }
    let body = if cfg!(target_os = "macos") { plist(&bin, port, &svc.home, &path_env, &log) } else { unit(&bin, port, &path_env, &log) };
    let file = svc.file();
    std::fs::create_dir_all(file.parent().context("service dir")?)?;
    let was_running = svc.installed() && svc.pid().is_some();
    if !was_running && port_busy(port) {
        // A dashboard started by hand holds the port: the service would only loop on bind errors.
        std::fs::write(&file, body)?;
        ui::warn(&format!("cổng {port} đang có dashboard khác chạy (bật tay bằng `ckit harness web`)."));
        ui::step("đã ghi cấu hình service nhưng CHƯA bật. Tắt dashboard đó (Ctrl+C / kill), rồi chạy `ckit service restart`.");
        ui::step("các bước Board đang chạy không bị cắt: dashboard mới sẽ tiếp tục theo dõi chúng.");
        return svc.load_deferred();
    }
    std::fs::write(&file, body)?;
    svc.load()?;
    ui::ok(&format!("service Board đã bật → http://127.0.0.1:{port}"));
    ui::step(&format!("tự chạy khi đăng nhập, tự khởi động lại khi lỗi. Log: {}", log.display()));
    ui::step(&format!("file: {}", file.display()));
    Ok(())
}

impl Service {
    /// Registered so it starts at the next login / `restart`, not started now.
    fn load_deferred(&self) -> Result<()> {
        if !cfg!(target_os = "macos") {
            run_cmd("systemctl", &["--user", "daemon-reload"])?;
            run_cmd("systemctl", &["--user", "enable", UNIT])?;
        }
        // macOS: the plist in ~/Library/LaunchAgents loads at next login; `restart` bootstraps it.
        Ok(())
    }
}

fn uninstall(svc: &Service) -> Result<()> {
    if !svc.installed() {
        ui::info("service chưa cài — không có gì để gỡ");
        return Ok(());
    }
    svc.unload()?;
    std::fs::remove_file(svc.file())?;
    if !cfg!(target_os = "macos") {
        let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
    }
    ui::ok("đã gỡ service Board (các bước đang chạy vẫn chạy tới hết; bật lại dashboard để theo dõi)");
    Ok(())
}

fn status(svc: &Service) -> Result<()> {
    ui::header("ckit service");
    if !svc.installed() {
        ui::info("chưa cài. `ckit service install` để Board tự chạy nền.");
        return Ok(());
    }
    match svc.pid() {
        Some(pid) => ui::ok(&format!("đang chạy (pid {pid})")),
        None => ui::warn("đã cài nhưng đang dừng — `ckit service restart`, xem `ckit service logs`"),
    }
    ui::step(&format!("file: {}", svc.file().display()));
    ui::step(&format!("log:  {}", svc.log().display()));
    Ok(())
}

fn logs(svc: &Service) -> Result<()> {
    let raw = std::fs::read_to_string(svc.log()).unwrap_or_default();
    let lines: Vec<&str> = raw.lines().collect();
    for l in &lines[lines.len().saturating_sub(40)..] {
        println!("{l}");
    }
    if lines.is_empty() {
        ui::info(&format!("chưa có log ({})", svc.log().display()));
    }
    Ok(())
}

fn run_cmd(bin: &str, args: &[&str]) -> Result<()> {
    let out = Command::new(bin).args(args).output().with_context(|| format!("{bin} không chạy được"))?;
    if !out.status.success() {
        bail!("{bin} {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

fn cmd_out(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_runs_the_dashboard_with_the_installers_path_escaped() {
        let p = plist(Path::new("/Users/a b/.local/bin/ckit"), 8799, Path::new("/Users/a b"), "/opt/x&y/bin:/usr/bin", Path::new("/Users/a b/.cache/ckit/board-service.log"));
        assert!(p.contains("<string>/Users/a b/.local/bin/ckit</string>\n    <string>harness</string>\n    <string>web</string>\n    <string>--no-open</string>\n    <string>--port</string>\n    <string>8799</string>"));
        assert!(p.contains("<string>/opt/x&amp;y/bin:/usr/bin</string>"), "PATH escaped");
        assert!(p.contains("<key>KeepAlive</key>\n  <true/>"));
        assert!(!p.contains("AbandonProcessGroup"), "launchd may kill the dashboard's group; Board runs have their own");
    }

    #[test]
    fn unit_kills_only_the_dashboard() {
        let u = unit(Path::new("/home/u/.local/bin/ckit"), 8731, "/home/u/.bun/bin:/usr/bin", Path::new("/home/u/.cache/ckit/board-service.log"));
        assert!(u.contains("ExecStart=\"/home/u/.local/bin/ckit\" harness web --no-open --port 8731"));
        assert!(u.contains("KillMode=process"));
        assert!(u.contains("Environment=\"PATH=/home/u/.bun/bin:/usr/bin\""));
    }

    #[test]
    fn pid_from_launchctl_and_systemctl() {
        assert_eq!(parse_pid("gui/501/ckit.board = {\n\tstate = running\n\tpid = 4242\n"), Some(4242));
        assert_eq!(parse_pid("gui/501/ckit.board = {\n\tstate = not running\n"), None);
        assert_eq!(parse_pid("0\n"), None);
        assert_eq!(parse_pid("777\n"), Some(777));
    }
}
