use std::env;
use std::process::ExitCode;

use agentscript_compiler::ParseFileError;

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
        Some(path) => match agentscript_compiler::parse_script_file(path) {
            Err(ParseFileError::Io(e)) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
            Err(ParseFileError::Compile(e)) => Err(e),
            Ok(s) => Ok(s),
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
