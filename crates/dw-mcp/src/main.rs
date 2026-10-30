use dw_mcp::McpServer;
use std::io::{BufRead, Write};

const USAGE: &str = "usage: dw-mcp [--max-open <N>]";

fn max_open(args: &[String]) -> Result<usize, String> {
    match args {
        [] => Ok(8),
        [flag, value] if flag == "--max-open" => {
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(format!(
                    "--max-open expects a base-10 integer, got {value:?}"
                ));
            }
            let n: usize = value
                .parse()
                .map_err(|_| format!("--max-open is out of range: {value}"))?;
            if n == 0 {
                return Err("--max-open must be at least 1".into());
            }
            Ok(n)
        }
        _ => Err(USAGE.into()),
    }
}

// One response line per message; each line is flushed before the next is read.
fn serve(max_open: usize) -> std::io::Result<()> {
    let mut server = McpServer::new(max_open);
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        if let Some(response) = server.handle(&line?) {
            writeln!(out, "{response}")?;
            out.flush()?;
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = max_open(&args).and_then(|n| serve(n).map_err(|e| e.to_string()));
    if let Err(message) = result {
        eprintln!("dw-mcp: {message}");
        std::process::exit(1);
    }
}
