use crate::internal_prelude::*;
use std::{
    net::SocketAddr,
    process::{Output, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    net::TcpStream,
    process::{Child, Command},
    sync::broadcast,
    time::sleep,
};

pub trait OutputExt {
    fn stderr(&self) -> String;
    fn has_stderr(&self) -> bool;
    fn stdout(&self) -> String;
    fn has_stdout(&self) -> bool;
}

impl OutputExt for Output {
    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.stderr).to_string()
    }

    fn has_stderr(&self) -> bool {
        println!("stderr: {}\n'{}'", self.stderr.len(), self.stderr());
        self.stderr.len() > 1
    }

    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.stdout).to_string()
    }

    fn has_stdout(&self) -> bool {
        self.stdout.len() > 1
    }
}
pub enum CommandResult<T> {
    Success(T),
    Failure(T),
    Interrupted,
}

/// Spawns `cmd` as the leader of a new process group, so that an interrupt can
/// stop everything it started and not only the process itself: cargo runs
/// rustc, build scripts and linkers as its children.
pub fn spawn_process_group(cmd: &mut Command) -> std::io::Result<Child> {
    #[cfg(unix)]
    cmd.process_group(0);
    cmd.spawn()
}

/// How long to wait for the processes of an interrupted group to go away
/// after they were killed. They cannot write anything once killed, so a group
/// that outlives this (a zombie its new parent does not reap) is left behind.
const GROUP_EXIT_TIMEOUT: Duration = Duration::from_secs(5);

/// Kills `process` and every process in its group, and waits until they are
/// gone. Killing only the leader would leave its children running: an
/// interrupted cargo would leave rustc writing outputs that the next build has
/// already used, which can leave the target directory inconsistent.
async fn kill_process_group(name: &str, process: &mut Child) -> Result<()> {
    trace!("Killing the {name} process and the processes it started");
    #[cfg(unix)]
    if let Some(pid) = process.id() {
        let group = pid as libc::pid_t;
        // SAFETY: `killpg` only sends a signal. The process is not yet reaped,
        // so its id, and the group id `spawn_process_group` gave it, cannot
        // have been reused.
        if unsafe { libc::killpg(group, libc::SIGKILL) } != 0 {
            // The process does not lead a group: it was not started with
            // `spawn_process_group`.
            return process.kill().await.wrap_err("Could not kill process");
        }
        process
            .wait()
            .await
            .wrap_err("Could not wait for process")?;
        let mut waited = Duration::ZERO;
        // SAFETY: signal 0 only checks whether a process of the group exists.
        while unsafe { libc::killpg(group, 0) } == 0 {
            if waited >= GROUP_EXIT_TIMEOUT {
                warn!(
                    "{name} processes were still present {GROUP_EXIT_TIMEOUT:?} after being killed"
                );
                break;
            }
            sleep(Duration::from_millis(10)).await;
            waited += Duration::from_millis(10);
        }
        return Ok(());
    }
    process.kill().await.wrap_err("Could not kill process")
}

pub async fn wait_interruptible(
    name: &str,
    mut process: Child,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> Result<CommandResult<()>> {
    trace!(?process, "Waiting for process to finish");
    tokio::select! {
        res = process.wait() => match res {
            Ok(exit) => {
                if exit.success() {
                    trace!("{name} process finished with success");
                    Ok(CommandResult::Success(()))
                } else {
                    trace!("{name} process finished with code {:?}", exit.code());
                    Ok(CommandResult::Failure(()))
                }
            }
            Err(e) => bail!("Command failed due to: {e}"),
        },
        _ = interrupt_rx.recv() => {
            kill_process_group(name, &mut process).await?;
            trace!("{name} process interrupted");
            Ok(CommandResult::Interrupted)
        }
    }
}

pub async fn wait_piped_interruptible(
    name: &str,
    mut cmd: Command,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> Result<CommandResult<Output>> {
    trace!(?cmd, "Waiting for command (piped)");
    // see: https://docs.rs/tokio/latest/tokio/process/index.html

    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut process = spawn_process_group(&mut cmd)?;
    // Read the pipes on their own tasks so that `process` stays available to
    // kill its group when interrupted.
    let stdout = tokio::spawn(read_to_end(process.stdout.take()));
    let stderr = tokio::spawn(read_to_end(process.stderr.take()));
    tokio::select! {
        res = process.wait() => match res {
            Ok(status) => {
                let output = Output {
                    status,
                    stdout: stdout.await.dot()?.dot()?,
                    stderr: stderr.await.dot()?.dot()?,
                };
                if output.status.success() {
                    trace!("{name} process finished with success");
                    Ok(CommandResult::Success(output))
                } else {
                    trace!("{name} process finished with code {:?}", output.status.code());
                    Ok(CommandResult::Failure(output))
                }
            }
            Err(e) => bail!("Command failed due to: {e}"),
        },
        _ = interrupt_rx.recv() => {
            kill_process_group(name, &mut process).await?;
            trace!("{name} process interrupted");
            Ok(CommandResult::Interrupted)
        }
    }
}

async fn read_to_end(pipe: Option<impl AsyncRead + Unpin>) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut buf).await?;
    }
    Ok(buf)
}

pub async fn wait_for_socket(name: &str, addr: SocketAddr) -> bool {
    let duration = Duration::from_millis(500);

    for _ in 0..20 {
        if TcpStream::connect(&addr).await.is_ok() {
            debug!("{name} server port {addr} open");
            return true;
        }
        sleep(duration).await;
    }
    warn!("{name} timed out waiting for port {addr}");
    false
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use temp_dir::TempDir;

    /// A shell that starts a background `sleep` and waits for it. Killing only
    /// the shell leaves the `sleep` running, the way killing only cargo leaves
    /// rustc running.
    fn shell_with_background_child(dir: &TempDir) -> Command {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(format!(
            "sleep 60 & echo $! > '{}'; wait",
            dir.path().join("child.pid").display()
        ));
        cmd
    }

    async fn background_child_pid(dir: &TempDir) -> i32 {
        for _ in 0..500 {
            if let Ok(pid) = std::fs::read_to_string(dir.path().join("child.pid")) {
                if let Ok(pid) = pid.trim().parse() {
                    return pid;
                }
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("the shell did not start its background child");
    }

    /// Kills `pid` if it still runs, so it cannot outlive the test, and returns
    /// whether it was running.
    fn kill_if_running(pid: i32) -> bool {
        // SAFETY: signal 0 only checks that the process exists.
        let running = unsafe { libc::kill(pid, 0) } == 0;
        if running {
            // SAFETY: only signals the test's own background child.
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        running
    }

    #[tokio::test]
    async fn interrupt_stops_the_processes_the_command_started() {
        let dir = TempDir::new().unwrap();
        let process = spawn_process_group(&mut shell_with_background_child(&dir)).unwrap();
        let child = background_child_pid(&dir).await;
        let (interrupt, interrupt_rx) = broadcast::channel(1);
        let waiter =
            tokio::spawn(async move { wait_interruptible("sh", process, interrupt_rx).await });

        interrupt.send(()).unwrap();

        let result = waiter.await.unwrap().unwrap();
        assert!(matches!(result, CommandResult::Interrupted));
        assert!(
            !kill_if_running(child),
            "the interrupted command's child {child} is still running"
        );
    }

    #[tokio::test]
    async fn interrupt_stops_a_process_that_does_not_lead_a_group() {
        let process = Command::new("sleep").arg("60").spawn().unwrap();
        let pid = process.id().unwrap() as i32;
        let (interrupt, interrupt_rx) = broadcast::channel(1);
        let waiter =
            tokio::spawn(async move { wait_interruptible("sleep", process, interrupt_rx).await });

        interrupt.send(()).unwrap();

        let result = tokio::time::timeout(Duration::from_secs(10), waiter).await;
        let was_running = kill_if_running(pid);
        let result = result.expect("waiting for the interrupted process did not return");
        assert!(matches!(
            result.unwrap().unwrap(),
            CommandResult::Interrupted
        ));
        assert!(
            !was_running,
            "the interrupted process {pid} is still running"
        );
    }

    #[tokio::test]
    async fn interrupt_stops_the_processes_a_piped_command_started() {
        let dir = TempDir::new().unwrap();
        let cmd = shell_with_background_child(&dir);
        let (interrupt, interrupt_rx) = broadcast::channel(1);
        let waiter =
            tokio::spawn(async move { wait_piped_interruptible("sh", cmd, interrupt_rx).await });
        let child = background_child_pid(&dir).await;

        interrupt.send(()).unwrap();

        let result = waiter.await.unwrap().unwrap();
        assert!(matches!(result, CommandResult::Interrupted));
        assert!(
            !kill_if_running(child),
            "the interrupted command's child {child} is still running"
        );
    }

    #[tokio::test]
    async fn a_piped_command_returns_its_output() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("echo out; echo err >&2; exit 3");
        let (_interrupt, interrupt_rx) = broadcast::channel(1);

        let result = wait_piped_interruptible("sh", cmd, interrupt_rx)
            .await
            .unwrap();

        let CommandResult::Failure(output) = result else {
            panic!("a command exiting with 3 must fail");
        };
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(output.stdout(), "out\n");
        assert_eq!(output.stderr(), "err\n");
    }
}
