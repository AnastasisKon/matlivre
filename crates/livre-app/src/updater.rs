//! Keeps the installed program up to date from its GitHub releases: checks for a newer release
//! in the background, downloads it and installs it when the program exits. Added by Release
//! Manager, which keeps REPOSITORY_URL and USE_GITHUB_SIGN_IN in step with its settings.

use velopack::{sources::GithubSource, UpdateCheck, UpdateManager};

/// The GitHub repository releases are published to.
pub const REPOSITORY_URL: &str = "https://github.com/campuslivre/matlivre";

/// Read releases with the GitHub sign-in already on the PC (git's credential store, then the
/// GitHub CLI), which a private repository needs. Off, releases are read anonymously.
pub const USE_GITHUB_SIGN_IN: bool = false;

/// Starts the check on a background thread and returns at once. Run from a build that was not
/// installed (cargo run), there is nothing to update and it does nothing.
pub fn check_in_background() {
    std::thread::spawn(|| {
        // Offline, rate-limited or a release still uploading: the next start tries again.
        let _ = check();
    });
}

fn check() -> Result<(), Box<dyn std::error::Error>> {
    let token = if USE_GITHUB_SIGN_IN { github_token() } else { None };
    // Pre-releases are left out: they reach installed copies once they are made the latest release.
    let manager = UpdateManager::new(GithubSource::new(REPOSITORY_URL, token, false), None, None)?;
    if let UpdateCheck::UpdateAvailable(update) = manager.check_for_updates()? {
        manager.download_updates(&update, None)?;
        // Installed after the program exits, so the user is never interrupted.
        manager.wait_exit_then_apply_updates(&*update, true, false, std::iter::empty::<&str>())?;
    }
    Ok(())
}

/// The GitHub token git's credential helper keeps for github.com, else the GitHub CLI's; None
/// when neither has one. Neither is allowed to open a sign-in window.
fn github_token() -> Option<String> {
    let stored = run("git", &["-c", "credential.interactive=never", "credential", "fill"], Some("protocol=https\nhost=github.com\n\n"))?;
    if let Some(password) = stored.lines().find_map(|l| l.strip_prefix("password=")) {
        return Some(password.trim().to_string());
    }
    run("gh", &["auth", "token"], None).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

fn run(program: &str, args: &[&str], input: Option<&str>) -> Option<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut command = Command::new(program);
    command.args(args).env("GIT_TERMINAL_PROMPT", "0").env("GCM_INTERACTIVE", "never")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.unwrap_or("").as_bytes());
    }
    let output = child.wait_with_output().ok()?;
    if output.status.success() { String::from_utf8(output.stdout).ok() } else { None }
}
