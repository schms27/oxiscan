//! Starts `oxiscan mcp` and talks MCP to it over stdio, as an AI client would.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Client {
    fn start(args: &[&str]) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oxiscan"))
            .arg("mcp")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("oxiscan starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        let init = c.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" }
            }),
        );
        assert_eq!(init["serverInfo"]["name"], "oxiscan");
        c.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        c
    }

    fn send(&mut self, msg: Value) {
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Sends a request and returns its result, skipping notifications such as progress.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "server closed"
            );
            let msg: Value = serde_json::from_str(&line).expect("one JSON message per line");
            if msg["id"] == id {
                assert!(msg.get("error").is_none(), "{method} failed: {msg}");
                return msg["result"].clone();
            }
        }
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": tool, "arguments": args, "_meta": { "progressToken": 7 } }),
        )
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn lists_tools() {
    let mut c = Client::start(&[]);
    let tools = c.request("tools/list", json!({}));
    let mut names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["list_saved", "local_network", "scan", "scan_host"]);
}

#[test]
fn scans_localhost() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut c = Client::start(&[]);

    let r = c.call(
        "scan",
        json!({ "target": "127.0.0.1", "ports": port.to_string() }),
    );
    assert_ne!(r["isError"], true, "{r}");
    let report = &r["structuredContent"];
    assert_eq!(report["addresses"], 1);
    assert_eq!(report["alive"], 1);
    assert_eq!(report["hosts"][0]["ip"], "127.0.0.1");
    assert_eq!(report["hosts"][0]["open_ports"][0]["port"], port);

    let r = c.call(
        "scan_host",
        json!({ "address": "127.0.0.1", "ports": port.to_string() }),
    );
    assert_ne!(r["isError"], true, "{r}");
    assert_eq!(r["structuredContent"]["open_ports"][0]["port"], port);
}

#[test]
fn refuses_public_addresses_and_large_scans() {
    let mut c = Client::start(&["--max-hosts", "16"]);
    let r = c.call("scan", json!({ "target": "8.8.8.8" }));
    assert_eq!(r["isError"], true);
    assert!(r.to_string().contains("--allow-public"), "{r}");

    let r = c.call("scan", json!({ "target": "10.0.0.0/24" }));
    assert_eq!(r["isError"], true);
    assert!(r.to_string().contains("at most 16"), "{r}");
}
