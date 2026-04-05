use std::env;
use std::fs;
use std::process::ExitCode;

use agentscript_compiler::CompileOrAnalyzeError;

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
                Ok(_) => agentscript_compiler::parse_and_analyze("<stdin>", &buf),
            }
        }
        Some(path) => match fs::read_to_string(path) {
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
            Ok(source) => agentscript_compiler::parse_and_analyze(path, &source),
        },
    };

    match parsed {
        Ok(script) => {
            println!("{script:#?}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            match err {
                CompileOrAnalyzeError::Parse(p) => eprintln!("{:?}", miette::Report::new(p)),
                CompileOrAnalyzeError::Analyze(a) => eprintln!("{a}"),
            }
            ExitCode::FAILURE
        }
    }
}
