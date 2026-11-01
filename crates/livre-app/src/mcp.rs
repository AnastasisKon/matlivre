//! MatLivre as an MCP server, so Claude Code can run code in the workspace, read variables and
//! read or edit the open documents.
//!
//! Each window serves MCP over HTTP on 127.0.0.1, at a URL holding a random token, and writes
//! that URL to `%APPDATA%\matlivre\mcp-url.txt`. Tool calls go to the UI thread, which owns
//! the documents and the engine. `matlivre --mcp` serves MCP over stdio for `claude mcp add`:
//! it forwards to a running window when there is one, and otherwise runs its own interpreter
//! without a window.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use livre_lang::{Figure, Output, OutputKind, VarInfo};
use serde_json::{json, Value};

/// The tool the chat panel's Claude asks before using a tool (`--permission-prompt-tool`).
pub const APPROVE: &str = "approve";

const PROTOCOLS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

const INSTRUCTIONS: &str = "MatLivre is a MATLAB-compatible desktop environment (MATLAB R2023b semantics). \
These tools act on the user's running MatLivre session: run_code runs MATLAB code in the base workspace and \
shows it in the command window, the document tools read and edit the scripts open in the editor (changes \
appear in the editor unsaved; the user saves them).";

/// A tool call handed to the UI thread, which answers on `reply`.
pub struct Request {
    pub tool: String,
    pub args: Value,
    pub reply: Sender<ToolResult>,
}

pub struct ToolResult {
    pub content: Vec<Value>,
    pub is_error: bool,
}

impl ToolResult {
    pub fn text(t: impl Into<String>) -> Self {
        ToolResult { content: vec![json!({ "type": "text", "text": t.into() })], is_error: false }
    }

    pub fn error(t: impl Into<String>) -> Self {
        ToolResult { content: vec![json!({ "type": "text", "text": t.into() })], is_error: true }
    }
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
    })
}

/// The tools on offer: all of them in a window (plus the permission prompt for its chat
/// panel), only the workspace ones without a window.
pub fn tools(window: bool, dock: bool) -> Vec<Value> {
    let doc = json!({ "type": "string", "description": "File name or full path of an open document. Omit for the active one." });
    let mut t = vec![
        tool(
            "run_code",
            "Run MATLAB code in MatLivre's base workspace, as if typed in the command window. Returns the command window output; figures come back as images.",
            json!({ "code": { "type": "string", "description": "MATLAB code; several lines allowed." } }),
            &["code"],
        ),
        tool("get_workspace", "List the variables in MatLivre's base workspace: name, class, size and a short value.", json!({}), &[]),
    ];
    if !window {
        return t;
    }
    t.extend([
        tool("list_documents", "List the documents open in MatLivre's editor: name, path, kind (script or live script) and whether they have unsaved changes.", json!({}), &[]),
        tool(
            "read_document",
            "Read an open document's text as the editor holds it, unsaved changes included. Live scripts are given in their plain .m form.",
            json!({ "document": doc }),
            &[],
        ),
        tool(
            "edit_document",
            "Replace text in an open document, in the editor (the user saves). old_text must occur exactly once.",
            json!({ "document": doc, "old_text": { "type": "string" }, "new_text": { "type": "string" } }),
            &["old_text", "new_text"],
        ),
        tool(
            "write_document",
            "Replace an open document's whole text, in the editor (the user saves). Live scripts take their plain .m form.",
            json!({ "document": doc, "text": { "type": "string" } }),
            &["text"],
        ),
        tool(
            "open_document",
            "Open a .m or .mlx file in MatLivre's editor, or create a new script when the path does not exist yet.",
            json!({ "path": { "type": "string", "description": "Full path of the file." } }),
            &["path"],
        ),
        tool(
            "run_document",
            "Run an open document, or one of its sections (1-based, split at %% lines), as the Run buttons do. Returns its output; figures come back as images.",
            json!({ "document": doc, "section": { "type": "integer", "minimum": 1 } }),
            &[],
        ),
    ]);
    if dock {
        t.push(tool(
            APPROVE,
            "Asks the MatLivre user whether a tool may run.",
            json!({ "tool_name": { "type": "string" }, "input": { "type": "object" }, "tool_use_id": { "type": "string" } }),
            &["tool_name", "input"],
        ));
    }
    t
}

/// Answers one JSON-RPC message; `None` for notifications.
pub fn handle(msg: &Value, tools: &[Value], call: &mut dyn FnMut(&str, Value) -> ToolResult) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = if PROTOCOLS.contains(&asked) { asked } else { "2025-06-18" };
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "matlivre", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            if !tools.iter().any(|t| t["name"] == name) {
                return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": format!("Unknown tool: {name}") } }));
            }
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let r = call(name, args);
            json!({ "content": r.content, "isError": r.is_error })
        }
        _ => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("Method not found: {method}") } })),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// Gathers a run's output for a tool result.
#[derive(Default)]
pub struct Collector {
    text: String,
    figures: Vec<Figure>,
}

impl Collector {
    pub fn push(&mut self, out: &Output) {
        match &out.kind {
            OutputKind::Text(t) => self.text.push_str(t),
            OutputKind::Value { text, .. } => {
                self.text.push_str(text);
                if !text.ends_with('\n') {
                    self.text.push('\n');
                }
            }
            OutputKind::Warning(w) => self.text.push_str(&format!("Warning: {w}\n")),
            OutputKind::Error(e) => self.text.push_str(&format!("Error (line {}): {e}\n", out.line)),
            OutputKind::Figure(f) => match self.figures.iter_mut().find(|g| g.id == f.id) {
                Some(g) => *g = f.clone(),
                None => self.figures.push(f.clone()),
            },
        }
    }

    /// The tool result: the text, then each figure as a PNG.
    pub fn finish(self, ctx: &egui::Context, ok: bool) -> ToolResult {
        let text = if self.text.trim().is_empty() { if ok { "(no output)".to_string() } else { "Stopped.".to_string() } } else { self.text };
        let mut content = vec![json!({ "type": "text", "text": text })];
        let skin = crate::theme::Skin::light();
        for f in &self.figures {
            if let Ok(png) = crate::plot::to_png(ctx, &skin, f, crate::plot::figure_size(560.0), 1.0) {
                content.push(json!({ "type": "image", "data": base64(&png), "mimeType": "image/png" }));
            }
        }
        ToolResult { content, is_error: !ok }
    }
}

pub fn workspace_text(vars: &[VarInfo]) -> String {
    if vars.is_empty() {
        return "The workspace is empty.".into();
    }
    vars.iter().map(|v| format!("{}  {} {}  {}", v.name, v.size, v.class, v.value)).collect::<Vec<_>>().join("\n")
}

pub fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                s.push(A[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn url_file() -> Option<PathBuf> {
    crate::app::app_dir().map(|d| d.join("mcp-url.txt"))
}

/// The HTTP server of one window.
pub struct Server {
    pub rx: Receiver<Request>,
    /// For Claude Code in a terminal (`claude mcp add --transport http matlivre <url>`).
    pub url: String,
    /// For the chat panel: also offers the permission prompt.
    pub dock_url: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        // Leave the file to a window started later.
        if let Some(f) = url_file() {
            if std::fs::read_to_string(&f).is_ok_and(|u| u.trim() == self.url) {
                let _ = std::fs::remove_file(f);
            }
        }
    }
}

fn token() -> String {
    let mut s = String::new();
    for _ in 0..2 {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        s.push_str(&format!("{:016x}", h.finish()));
    }
    s
}

pub fn start(ctx: egui::Context) -> Option<Server> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    let token = token();
    let url = format!("http://127.0.0.1:{port}/mcp/{token}");
    let dock_url = format!("{url}/dock");
    let (tx, rx) = channel();
    let (base, dock) = (format!("/mcp/{token}"), format!("/mcp/{token}/dock"));
    std::thread::Builder::new()
        .name("livre-mcp".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let (tx, ctx, base, dock) = (tx.clone(), ctx.clone(), base.clone(), dock.clone());
                std::thread::spawn(move || serve(stream, &base, &dock, &tx, &ctx));
            }
        })
        .ok()?;
    if let Some(f) = url_file().filter(|_| !cfg!(test)) {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, &url);
    }
    Some(Server { rx, url, dock_url })
}

struct HttpRequest {
    method: String,
    path: String,
    origin: Option<String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut impl Read) -> Option<HttpRequest> {
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_string(), parts.next()?.to_string());
    let (mut len, mut origin) = (0usize, None);
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).ok()? == 0 {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let (k, v) = h.split_once(':')?;
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => len = v.trim().parse().ok()?,
            "origin" => origin = Some(v.trim().to_string()),
            _ => {}
        }
    }
    let mut body = vec![0; len.min(64 << 20)];
    r.read_exact(&mut body).ok()?;
    Some(HttpRequest { method, path, origin, body })
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let kind = if body.is_empty() { "" } else { "Content-Type: application/json\r\n" };
    let _ = write!(stream, "HTTP/1.1 {status}\r\n{kind}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
}

fn serve(mut stream: TcpStream, base: &str, dock: &str, tx: &Sender<Request>, ctx: &egui::Context) {
    let Some(req) = read_request(&mut stream) else { return };
    let path = req.path.split('?').next().unwrap_or("");
    let is_dock = path == dock;
    // Web pages may reach localhost too; only programs, which send no Origin, get in.
    if !(path == base || is_dock) || req.origin.is_some_and(|o| o != "null") {
        return respond(&mut stream, "403 Forbidden", "");
    }
    if req.method != "POST" {
        return respond(&mut stream, "405 Method Not Allowed", "");
    }
    let Ok(msg) = serde_json::from_slice::<Value>(&req.body) else {
        return respond(&mut stream, "400 Bad Request", "");
    };
    let tools = tools(true, is_dock);
    let mut call = |name: &str, args: Value| {
        let (reply, answer) = channel();
        if tx.send(Request { tool: name.to_string(), args, reply }).is_err() {
            return ToolResult::error("MatLivre is closing.");
        }
        ctx.request_repaint();
        answer.recv().unwrap_or_else(|_| ToolResult::error("MatLivre dropped the request."))
    };
    let answer = match &msg {
        Value::Array(batch) => {
            let out: Vec<Value> = batch.iter().filter_map(|m| handle(m, &tools, &mut call)).collect();
            (!out.is_empty()).then(|| Value::Array(out))
        }
        m => handle(m, &tools, &mut call),
    };
    match answer {
        Some(v) => respond(&mut stream, "200 OK", &v.to_string()),
        None => respond(&mut stream, "202 Accepted", ""),
    }
}

/// POSTs a JSON-RPC message to a window's server; `Some("")` for an accepted notification.
fn forward(url: &str, body: &str) -> Option<String> {
    let rest = url.strip_prefix("http://")?;
    let (host, path) = rest.split_once('/')?;
    let addr = host.parse().ok()?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(500)).ok()?;
    write!(s, "POST /{path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).ok()?;
    let mut resp = String::new();
    s.read_to_string(&mut resp).ok()?;
    let (head, body) = resp.split_once("\r\n\r\n")?;
    let ok = head.split_whitespace().nth(1).is_some_and(|c| c.starts_with('2'));
    ok.then(|| body.to_string())
}

/// `matlivre --mcp`: MCP over stdio, one JSON-RPC message per line.
pub fn stdio() {
    let window = url_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|u| u.trim().to_string());
    let mut local: Option<Headless> = None;
    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match window.as_deref().and_then(|u| forward(u, &line)) {
            Some(body) => (!body.trim().is_empty()).then_some(body),
            None => {
                let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
                let h = local.get_or_insert_with(Headless::new);
                let tools = tools(false, false);
                handle(&msg, &tools, &mut |name, args| h.call(name, &args)).map(|v| v.to_string())
            }
        };
        if let Some(r) = reply {
            let mut out = stdout.lock();
            let _ = writeln!(out, "{}", r.trim());
            let _ = out.flush();
        }
    }
}

/// An interpreter without a window, for `--mcp` when no MatLivre window is open.
struct Headless {
    it: livre_lang::Interpreter,
    outs: std::rc::Rc<std::cell::RefCell<Vec<Output>>>,
    ctx: egui::Context,
}

impl Headless {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        // Fonts exist once a frame has run; figures need them for their text.
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let mut it = livre_lang::Interpreter::new();
        let outs = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = outs.clone();
        it.set_output_sink(move |o| sink.borrow_mut().extend(o));
        crate::engine::install_exporter(&mut it, ctx.clone());
        Headless { it, outs, ctx }
    }

    fn call(&mut self, name: &str, args: &Value) -> ToolResult {
        match name {
            "get_workspace" => ToolResult::text(workspace_text(&self.it.workspace())),
            "run_code" => {
                let code = args.get("code").and_then(Value::as_str).unwrap_or("");
                let ok = match livre_lang::parser::parse(code) {
                    Ok(prog) => {
                        self.it.define_functions(&prog);
                        self.it.run(&prog.body).is_ok()
                    }
                    Err(e) => {
                        self.outs.borrow_mut().push(Output { line: e.line.max(1), kind: OutputKind::Error(e.message) });
                        false
                    }
                };
                let mut c = Collector::default();
                for o in self.outs.borrow_mut().drain(..) {
                    c.push(&o);
                }
                c.finish(&self.ctx, ok)
            }
            _ => ToolResult::error(format!("{name} needs a MatLivre window.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call_none(_: &str, _: Value) -> ToolResult {
        ToolResult::text("called")
    }

    #[test]
    fn initialize_list_and_call() {
        let list_of = tools;
        let tools = list_of(true, false);
        let init = handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}), &tools, &mut call_none).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "matlivre");
        assert!(handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}), &tools, &mut call_none).is_none());
        let list = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}), &tools, &mut call_none).unwrap();
        let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"run_code") && names.contains(&"edit_document") && !names.contains(&APPROVE));
        let mut seen = String::new();
        let r = handle(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"run_code","arguments":{"code":"x=1"}}}), &tools, &mut |n, a| {
            seen = format!("{n} {}", a["code"]);
            ToolResult::text("ok")
        })
        .unwrap();
        assert_eq!(seen, "run_code \"x=1\"");
        assert_eq!(r["result"]["content"][0]["text"], "ok");
        assert_eq!(r["result"]["isError"], false);
        let bad = handle(&json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"approve"}}), &tools, &mut call_none).unwrap();
        assert_eq!(bad["error"]["code"], -32602);
        assert!(list_of(true, true).iter().any(|t| t["name"] == APPROVE));
        assert_eq!(list_of(false, false).len(), 2);
    }

    #[test]
    fn base64_encodes() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn http_server_round_trip() {
        let ctx = egui::Context::default();
        let mut server = start(ctx).unwrap();
        // A tool call is answered by whoever holds the receiver (the UI thread in the app).
        let rx = std::mem::replace(&mut server.rx, channel().1);
        let answer = std::thread::spawn(move || {
            let r = rx.recv().unwrap();
            assert_eq!(r.tool, "get_workspace");
            let _ = r.reply.send(ToolResult::text("a  1x1 double  5"));
        });
        let body = forward(&server.url, r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"get_workspace","arguments":{}}}"#).unwrap();
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["result"]["content"][0]["text"], "a  1x1 double  5");
        answer.join().unwrap();
        assert_eq!(forward(&server.url, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).as_deref(), Some(""));
        // A wrong token is refused.
        assert!(forward(&server.url.replace("/mcp/", "/mcp/x"), r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).is_none());
    }
}
