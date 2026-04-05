use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("-") | None => {
            let mut buf = String::new();
            use std::io::Read;
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| e.to_string())
                .and_then(|_| agentscript_compiler::parse_script("<stdin>", &buf).map_err(|e| e.to_string()))
        }
        Some(path) => fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| agentscript_compiler::parse_script(path, &s).map_err(|e| e.to_string())),
    };

    match result {
        Ok(script) => {
            println!("{script:#?}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}
