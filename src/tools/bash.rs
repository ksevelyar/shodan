use std::{io, process::Stdio};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

const OUTPUT_LIMIT: usize = 50 * 1024;

pub(super) async fn run_command(command: &str) -> io::Result<(String, bool)> {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .process_group(0)
        .spawn()?;
    let _process_group = ProcessGroup {
        process_id: child.id(),
    };
    let standard_output = child.stdout.take().expect("the command pipes its output");
    let standard_error = child.stderr.take().expect("the command pipes its errors");
    let standard_output_task = tokio::spawn(read_limited(standard_output));
    let standard_error_task = tokio::spawn(read_limited(standard_error));
    let (standard_output, standard_output_truncated) = standard_output_task.await??;
    let (standard_error, standard_error_truncated) = standard_error_task.await??;
    let status = child.wait().await?;
    let mut output = String::from_utf8_lossy(&standard_output).into_owned();
    let errors = String::from_utf8_lossy(&standard_error);
    if !errors.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&errors);
    }
    if standard_output_truncated || standard_error_truncated {
        output.push_str("\n[output truncated]");
    }
    if !status.success() {
        let exit_description = match status.code() {
            Some(code) => format!("exit code {code}"),
            None => "a signal".to_owned(),
        };
        output.push_str("\nCommand failed with ");
        output.push_str(&exit_description);
    }
    Ok((output, status.success()))
}

async fn read_limited(
    mut stream: impl AsyncRead + Unpin + Send + 'static,
) -> io::Result<(Vec<u8>, bool)> {
    let mut output = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 4096];
    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Ok((output, truncated));
        }
        let remaining = OUTPUT_LIMIT.saturating_sub(output.len());
        output.extend_from_slice(&buffer[..read.min(remaining)]);
        if read > remaining {
            truncated = true;
        }
    }
}

struct ProcessGroup {
    process_id: Option<u32>,
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        if let Some(process_id) = self.process_id {
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(format!("-{process_id}"))
                .stderr(Stdio::null())
                .status();
        }
    }
}
