use deckwright::{Limits, Package};
fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    match a.as_slice() {
        [cmd, input, json] if cmd == "inspect" && json == "--json" => {
            let p = Package::open(input, Limits::default())?;
            serde_json::to_writer(std::io::stdout().lock(), &p.inventory())?;
            println!();
        }
        [cmd, input, flag, output] if cmd == "roundtrip" && flag == "--output" => {
            if std::path::Path::new(input).canonicalize().ok()
                == std::path::Path::new(output).canonicalize().ok()
                && std::path::Path::new(output).exists()
            {
                return Err("input and output must differ".into());
            }
            Package::open(input, Limits::default())?.write(output)?;
        }
        [cmd, input, json] if cmd == "check" && json == "--json" => {
            let report = deckwright::check::check_path(input, Limits::default());
            serde_json::to_writer(std::io::stdout().lock(), &report)?;
            println!();
            return Ok(i32::from(report.has_errors()));
        }
        _ => {
            return Err(
                "usage: dw inspect <pptx> --json | dw roundtrip <input> --output <new-output> | dw check <pptx> --json"
                    .into(),
            );
        }
    }
    Ok(0)
}
fn main() {
    match run() {
        Ok(0) => {}
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("dw: {e}");
            std::process::exit(1);
        }
    }
}
