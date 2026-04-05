use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let parsed = match args.next().as_deref() {
        Some("-") | None => {
            let mut buf = String::new();
            use std::io::Read;
            match std::io::stdin().read_to_string(&mut buf) {
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
                Ok(_) => agentscript_compiler::parse_script("<stdin>", &buf),
            }
        }
        Some(path) => match fs::read_to_string(path) {
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
            Ok(s) => agentscript_compiler::parse_script(path, &s),
        },
    };

    match parsed {
        Ok(script) => {
            println!("{script:#?}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{:?}", miette::Report::new(err));
            ExitCode::FAILURE
        }
    }
}
