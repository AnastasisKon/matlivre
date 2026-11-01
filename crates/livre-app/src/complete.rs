//! Inline code completion from the Claude Code CLI. Starting `claude` takes seconds, so one
//! process is always started ahead and waits for its prompt; each completion uses it once,
//! with a fresh context, and the next one starts at the same time.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use eframe::egui;
use serde_json::Value;

const SYSTEM: &str = "You are the code completion engine of a MATLAB editor. You get the file being edited with \
<CURSOR> marking the caret. Reply with only the text to insert at <CURSOR>: no explanation, no markdown fences, \
nothing that is already before or after the caret. Finish the current line and, when it is natural, the rest of \
the statement or block (at most 10 lines). Use MATLAB syntax and the variables in scope. If nothing fits, reply <none>.";

/// Characters of context sent before and after the caret.
const BEFORE: usize = 6000;
const AFTER: usize = 1500;

/// A prompt for the text around the caret.
pub fn prompt(file: &str, before: &str, after: &str, vars: &[String]) -> String {
    let start = before.char_indices().rev().nth(BEFORE - 1).map_or(0, |(i, _)| i);
    let end = after.char_indices().nth(AFTER).map_or(after.len(), |(i, _)| i);
    let mut p = format!("File: {file}\n");
    if !vars.is_empty() {
        p.push_str(&format!("Workspace variables: {}\n", vars.join(", ")));
    }
    p.push_str(&format!("<file>\n{}<CURSOR>{}\n</file>", &before[start..], &after[..end]));
    p
}

/// The insertion in a reply: fences and text repeated from around the caret removed.
pub fn clean(reply: &str, before: &str, after: &str) -> String {
    let mut s = reply.trim_end().to_string();
    if s.trim() == "<none>" {
        return String::new();
    }
    if let Some(rest) = s.trim_start().strip_prefix("```") {
        let body = rest.split_once('\n').map_or("", |(_, b)| b);
        s = body.trim_end().strip_suffix("```").unwrap_or(body).trim_end().to_string();
    }
    s = s.replace("<CURSOR>", "");
    // A reply that restates the current line from its start.
    let line = before.rsplit('\n').next().unwrap_or("");
    if !line.trim().is_empty() {
        if let Some(rest) = s.strip_prefix(line) {
            s = rest.to_string();
        } else if let Some(rest) = s.strip_prefix(line.trim_start()) {
            s = rest.to_string();
        }
    }
    // Text after the caret the reply ends with (a closing bracket, say).
    let next = after.split('\n').next().unwrap_or("").trim_end();
    if !next.is_empty() {
        if let Some(head) = s.strip_suffix(next) {
            s = head.to_string();
        }
    }
    let lines: Vec<&str> = s.split('\n').take(12).collect();
    lines.join("\n").trim_end().to_string()
}

struct Warm {
    child: Arc<Mutex<Child>>,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

pub struct Completer {
    exe: Option<Option<PathBuf>>,
    spare: Option<Warm>,
    running: Option<(u64, Arc<Mutex<Child>>)>,
    tx: Sender<(u64, String)>,
    rx: Receiver<(u64, String)>,
}

impl Drop for Completer {
    fn drop(&mut self) {
        self.cancel();
        if let Some(w) = self.spare.take() {
            let _ = w.child.lock().map(|mut c| c.kill());
        }
    }
}

impl Completer {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Completer { exe: None, spare: None, running: None, tx, rx }
    }

    fn spawn(&mut self) -> Option<Warm> {
        let exe = self.exe.get_or_insert_with(crate::claude::find).clone()?;
        let mut cmd = crate::claude::command(&exe);
        cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"])
            .args(["--model", "haiku", "--effort", "low", "--tools", ""])
            .args(["--system-prompt", SYSTEM])
            .args(["--no-session-persistence", "--strict-mcp-config", "--disable-slash-commands"])
            .env("MAX_THINKING_TOKENS", "0")
            .current_dir(std::env::temp_dir())
            .stderr(std::process::Stdio::null());
        let mut child = cmd.spawn().ok()?;
        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        Some(Warm { child: Arc::new(Mutex::new(child)), stdin, stdout })
    }

    /// Starts the process the next completion will use, so it is ready by then.
    pub fn warm_up(&mut self) {
        let dead = self.spare.as_ref().is_some_and(|w| w.child.lock().map(|mut c| !matches!(c.try_wait(), Ok(None))).unwrap_or(true));
        if dead {
            self.spare = None;
        }
        if self.spare.is_none() {
            self.spare = self.spawn();
        }
    }

    /// Asks for a completion; its answer comes back from [`Completer::poll`] under `key`.
    pub fn request(&mut self, key: u64, prompt: String, ctx: &egui::Context) {
        self.cancel();
        self.warm_up();
        let Some(mut w) = self.spare.take() else { return };
        self.running = Some((key, w.child.clone()));
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut answer = String::new();
            if w.stdin.write_all(crate::claude::user_message(&prompt).as_bytes()).and_then(|()| w.stdin.flush()).is_ok() {
                for line in BufReader::new(&mut w.stdout).lines() {
                    let Ok(line) = line else { break };
                    let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                    match v["type"].as_str() {
                        // The reply's text comes in the assistant message, before the result.
                        Some("assistant") => {
                            for b in v["message"]["content"].as_array().into_iter().flatten() {
                                if b["type"] == "text" {
                                    answer.push_str(b["text"].as_str().unwrap_or(""));
                                }
                            }
                            if !answer.is_empty() {
                                break;
                            }
                        }
                        Some("result") => {
                            if answer.is_empty() {
                                answer = v["result"].as_str().unwrap_or("").to_string();
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
            drop(w.stdin);
            let _ = w.child.lock().map(|mut c| {
                let _ = c.kill();
                c.wait()
            });
            let _ = tx.send((key, answer));
            ctx.request_repaint();
        });
        self.warm_up();
    }

    pub fn cancel(&mut self) {
        if let Some((_, child)) = self.running.take() {
            let _ = child.lock().map(|mut c| c.kill());
        }
    }

    pub fn poll(&mut self) -> Option<(u64, String)> {
        let r = self.rx.try_iter().last()?;
        if self.running.as_ref().is_some_and(|(k, _)| *k == r.0) {
            self.running = None;
        }
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_are_cleaned() {
        assert_eq!(clean("y)", "plot(x, ", ""), "y)");
        assert_eq!(clean("```matlab\ny);\n```", "plot(x, ", ""), "y);");
        assert_eq!(clean("plot(x, y);", "    plot(x, ", ""), "y);");
        assert_eq!(clean("y)", "plot(x, ", ")"), "y");
        assert_eq!(clean("<none>", "x = 1;", ""), "");
        assert_eq!(clean("1:10\n    disp(i)\nend\n", "for i = ", ""), "1:10\n    disp(i)\nend");
    }

    /// A completion from the installed Claude Code, through a process started ahead.
    /// `cargo test -p livre-app real_completion -- --ignored`.
    #[test]
    #[ignore]
    fn real_completion() {
        let ctx = eframe::egui::Context::default();
        let mut c = Completer::new();
        c.warm_up();
        std::thread::sleep(std::time::Duration::from_secs(10));
        let (before, after) = ("x = linspace(0, 2*pi, 100);\ny = sin(x);\nplot(x, ", "");
        let started = std::time::Instant::now();
        c.request(1, prompt("demo.m", before, after, &[]), &ctx);
        let answer = loop {
            if let Some((1, a)) = c.poll() {
                break a;
            }
            assert!(started.elapsed().as_secs() < 60, "no answer");
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let text = clean(&answer, before, after);
        eprintln!("{text:?} in {:?}", started.elapsed());
        assert!(text.starts_with('y'), "{text:?}");
    }

    #[test]
    fn prompt_marks_the_caret() {
        let p = prompt("demo.m", "x = 1;\ny = ", "\nz = 3;", &["x".into()]);
        assert!(p.contains("Workspace variables: x"));
        assert!(p.contains("x = 1;\ny = <CURSOR>\nz = 3;"));
        let long = "a".repeat(BEFORE + 100);
        assert!(prompt("f.m", &long, "", &[]).contains(&format!("\n{}<CURSOR>", "a".repeat(BEFORE))));
    }
}
