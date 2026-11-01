//! Bounded Git subprocess execution. Git and its configured filters are trusted,
//! but may hang or cause external effects. Killing a process group is not a
//! rollback and cannot control descendants that deliberately escape it.
use anyhow::{bail, ensure, Context, Result};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

const GIT_DEADLINE: Duration = Duration::from_secs(60);
const DESCENDANT_GRACE: Duration = Duration::from_secs(2);
const MAX_STREAM_BYTES: u64 = 64 * 1024 * 1024;

/// Run a single Git command with bounded time and output. Input is written to a
/// seekable anonymous file before spawn, avoiding blocked pipe-writer threads;
/// both output streams are file-backed, so neither can deadlock the other.
pub(crate) fn run(command: &mut Command, input: Option<&[u8]>) -> Result<Output> {
    run_bounded(
        command,
        input,
        GIT_DEADLINE,
        MAX_STREAM_BYTES,
        DESCENDANT_GRACE,
    )
}

fn read_capture(mut file: File, limit: u64) -> Result<Vec<u8>> {
    ensure!(
        file.metadata()?.len() <= limit,
        "Git output exceeded the capture limit; effects may be uncertain"
    );
    file.seek(SeekFrom::Start(0))?;
    let mut data = Vec::new();
    file.take(limit + 1).read_to_end(&mut data)?;
    ensure!(
        data.len() as u64 <= limit,
        "Git output exceeded the capture limit; effects may be uncertain"
    );
    Ok(data)
}

fn run_bounded(
    command: &mut Command,
    input: Option<&[u8]>,
    deadline: Duration,
    output_limit: u64,
    descendant_grace: Duration,
) -> Result<Output> {
    let stdout = tempfile::tempfile().context("create Git stdout capture")?;
    let stderr = tempfile::tempfile().context("create Git stderr capture")?;
    let input_file = if let Some(bytes) = input {
        // Git path batches are already bounded by the caller, but ensure a
        // malformed input cannot consume unlimited temporary storage.
        ensure!(
            bytes.len() as u64 <= output_limit,
            "Git input exceeds the capture limit"
        );
        let mut f = tempfile::tempfile().context("create Git stdin")?;
        f.write_all(bytes).context("write Git stdin")?;
        f.seek(SeekFrom::Start(0))?;
        Some(f)
    } else {
        None
    };
    command
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?));
    command.stdin(input_file.map_or_else(Stdio::null, Stdio::from));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().context("start Git process")?;
    #[cfg(unix)]
    let group = child.id() as libc::pid_t;
    let stop = |child: &mut std::process::Child| {
        // Only signal the group before this child has been reaped: its PID
        // cannot yet have been reused by an unrelated process.
        #[cfg(unix)]
        if unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) } != 0 {
            let _ = child.kill();
        }
        #[cfg(not(unix))]
        let _ = child.kill();
        let reap_deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) if Instant::now() < reap_deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                // A leader stuck in the kernel or consumed by an external
                // reaper must not turn the timeout into another unbounded wait.
                _ => return false,
            }
        }
    };
    let end = Instant::now() + deadline;
    let status = loop {
        let size = match stdout
            .metadata()
            .and_then(|out| stderr.metadata().map(|err| out.len().max(err.len())))
        {
            Ok(size) => size,
            Err(error) => {
                let reaped = stop(&mut child);
                return Err(error).with_context(|| format!(
                    "inspect Git output; termination attempted (leader reaped: {reaped}); processes and external effects remain uncertain"
                ));
            }
        };
        if size > output_limit {
            let reaped = stop(&mut child);
            bail!("Git output exceeded {output_limit} bytes; process-group termination attempted (leader reaped: {reaped}); processes and external effects remain uncertain");
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => (),
            // A concurrent reaper could have consumed the child. Do not signal
            // an identity that might already have been reused.
            Err(error) => {
                return Err(error)
                    .context("wait for Git; process and external effects may be uncertain")
            }
        }
        if Instant::now() >= end {
            let reaped = stop(&mut child);
            bail!("Git command timed out after {} seconds; process-group termination attempted (leader reaped: {reaped}); processes and external effects remain uncertain", deadline.as_secs());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    #[cfg(unix)]
    {
        // try_wait has reaped the leader. Only probe now: never signal a group
        // that could have been reused after reaping. A lingering group is an
        // uncertain result, not a successful Git operation.
        let end = Instant::now() + descendant_grace;
        loop {
            let rc = unsafe { libc::kill(-group, 0) };
            let error = std::io::Error::last_os_error();
            if rc != 0 && error.raw_os_error() == Some(libc::ESRCH) {
                break;
            }
            if Instant::now() >= end {
                bail!("Git left processes in its process group; result and external effects remain uncertain");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[cfg(not(unix))]
    let _ = descendant_grace;
    Ok(Output {
        status,
        stdout: read_capture(stdout, output_limit)?,
        stderr: read_capture(stderr, output_limit)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_input_and_both_outputs_do_not_deadlock() {
        let bytes = vec![b'x'; 512 * 1024];
        let mut cmd = Command::new("python3");
        cmd.args(["-c", "import sys; b=sys.stdin.buffer.read(); sys.stderr.buffer.write(b); sys.stdout.buffer.write(b)"]);
        let out = run_bounded(
            &mut cmd,
            Some(&bytes),
            Duration::from_secs(5),
            1024 * 1024,
            Duration::from_secs(1),
        )
        .unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout, bytes);
        assert_eq!(out.stderr, bytes);
    }
    #[test]
    fn timeout_terminates_owned_group_without_leaving_a_child() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("late");
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("(sleep 1; touch \"$MARKER\") & wait")
            .env("MARKER", &marker);
        let error = run_bounded(
            &mut cmd,
            None,
            Duration::from_millis(250),
            1024 * 1024,
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("timed out"));
        std::thread::sleep(Duration::from_millis(1200));
        assert!(!marker.exists());
    }
    #[test]
    fn oversized_stdout_and_stderr_are_bounded() {
        for fd in ["1", "2"] {
            let mut cmd = Command::new("python3");
            cmd.args([
                "-c",
                &format!("import os,time; os.ftruncate({fd}, 1000000); time.sleep(30)"),
            ]);
            let error = run_bounded(
                &mut cmd,
                None,
                Duration::from_secs(5),
                1024,
                Duration::from_secs(1),
            )
            .unwrap_err();
            assert!(format!("{error:#}").contains("exceeded"));
        }
    }
    #[test]
    fn same_group_descendant_after_leader_exit_is_uncertain() {
        let mut cmd = Command::new("python3");
        cmd.args(["-c", "import subprocess; subprocess.Popen(['sleep','2'], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)"]);
        let error = run_bounded(
            &mut cmd,
            None,
            Duration::from_secs(5),
            1024 * 1024,
            Duration::from_millis(200),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("processes in its process group"));
        // Let the bounded test child exit before the process/test ends.
        std::thread::sleep(Duration::from_secs(2));
    }
}
