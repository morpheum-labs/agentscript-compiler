use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use agentscript_compiler::{
    analyze_to_hir_compiler, compile_script_to_wasm_v0, check_script, parse_script, AnalyzeError,
    CompileError, Span,
};

#[derive(Clone, Copy, Debug, Default)]
enum EmitKind {
    #[default]
    Ast,
    Hir,
    Wasm,
}

impl EmitKind {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "ast" => Some(Self::Ast),
            "hir" => Some(Self::Hir),
            "wasm" => Some(Self::Wasm),
            _ => None,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(RunError::Parse(p)) => {
            eprintln!("{:?}", miette::Report::new(p));
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

enum RunError {
    Io(std::io::Error),
    Parse(CompileError),
    Analyze(AnalyzeError),
    BadEmit(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Io(e) => write!(f, "{e}"),
            RunError::Parse(_) => write!(f, "parse error"),
            RunError::Analyze(a) => write!(f, "{a}"),
            RunError::BadEmit(s) => write!(f, "unknown --emit= value `{s}` (use ast, hir, or wasm)"),
        }
    }
}

impl From<std::io::Error> for RunError {
    fn from(e: std::io::Error) -> Self {
        RunError::Io(e)
    }
}

fn run() -> Result<(), RunError> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut emit = EmitKind::Ast;
    let mut path_arg: Option<&str> = None;
    for a in &args {
        if let Some(v) = a.strip_prefix("--emit=") {
            emit = EmitKind::parse(v).ok_or_else(|| RunError::BadEmit(v.to_string()))?;
        } else if !a.starts_with("--") {
            path_arg = Some(a.as_str());
        }
    }

    let (label, source) = match path_arg {
        None | Some("-") => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            ("<stdin>".to_string(), buf)
        }
        Some(path) => (path.to_string(), fs::read_to_string(path)?),
    };

    let script = parse_script(&label, &source).map_err(RunError::Parse)?;
    check_script(&script).map_err(RunError::Analyze)?;

    match emit {
        EmitKind::Ast => {
            println!("{script:#?}");
        }
        EmitKind::Hir => {
            let c = analyze_to_hir_compiler(&script).map_err(RunError::Analyze)?;
            let hir = c
                .session
                .hir
                .as_ref()
                .expect("HIR present after successful lowering pass");
            println!("{hir:#?}");
        }
        EmitKind::Wasm => {
            let wasm = compile_script_to_wasm_v0(&script).map_err(|e| {
                RunError::Analyze(AnalyzeError::single(e.to_string(), Span::DUMMY))
            })?;
            io::stdout().write_all(&wasm)?;
        }
    }

    Ok(())
}
