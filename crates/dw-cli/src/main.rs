use deckwright::{Limits, Package};
fn run() -> Result<(), Box<dyn std::error::Error>> {
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
        _ => {
            return Err(
                "usage: dw inspect <pptx> --json | dw roundtrip <input> --output <new-output>"
                    .into(),
            );
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("dw: {e}");
        std::process::exit(1);
    }
}
