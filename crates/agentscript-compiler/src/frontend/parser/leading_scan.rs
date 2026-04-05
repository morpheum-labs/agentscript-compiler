//! Leading trivia: validate `//@version=` (Pine) and `// @agentscript=` before the main parse.

use chumsky::error::Simple;

use super::version_policy::{qas_version_allowed, qas_version_unsupported_message};

/// Walk leading whitespace, `//` lines, and `/* */` blocks; validate any `//@version=` (Pine 5/6) or
/// `// @agentscript=` (≥1) in that trivia. Stops at the first non-trivia byte.
pub(crate) fn scan_leading_bad_directives(source: &str) -> Option<Simple<char>> {
    let mut i = 0usize;
    let b = source.as_bytes();
    while i < b.len() {
        let c = b[i];
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
            continue;
        }
        if i + 1 < b.len() && c == b'/' && b[i + 1] == b'/' {
            let line_start = i;
            let mut j = i + 2;
            while j < b.len() && b[j] != b'\n' && b[j] != b'\r' {
                j += 1;
            }
            let line = source.get(line_start..j).unwrap_or("");
            if let Some(_rest) = line.strip_prefix("//@version=") {
                let num_start = line_start + "//@version=".len();
                let mut num_end = num_start;
                for ch in source.get(num_start..j)?.chars() {
                    if ch.is_ascii_digit() {
                        num_end += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if num_end == num_start {
                    return Some(Simple::custom(
                        num_start..num_start.saturating_add(1).min(source.len()),
                        "missing version number after //@version=",
                    ));
                }
                let digits = source.get(num_start..num_end).unwrap_or("0");
                let n: u32 = digits.parse().unwrap_or(0);
                if !qas_version_allowed(n) {
                    return Some(Simple::custom(
                        num_start..num_end,
                        qas_version_unsupported_message(),
                    ));
                }
            } else if let Some(after_cc) = line.strip_prefix("//") {
                let bytes = after_cc.as_bytes();
                let mut k = 0usize;
                while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                    k += 1;
                }
                const AS_TAG: &str = "@agentscript=";
                if k > 0 && after_cc[k..].starts_with(AS_TAG) {
                    let num_start = line_start + 2 + k + AS_TAG.len();
                    let mut num_end = num_start;
                    for ch in source.get(num_start..j)?.chars() {
                        if ch.is_ascii_digit() {
                            num_end += ch.len_utf8();
                        } else {
                            break;
                        }
                    }
                    if num_end == num_start {
                        return Some(Simple::custom(
                            num_start..num_start.saturating_add(1).min(source.len()),
                            "missing number after // @agentscript=",
                        ));
                    }
                    let digits = source.get(num_start..num_end).unwrap_or("0");
                    let n: u32 = digits.parse().unwrap_or(0);
                    if n < 1 {
                        return Some(Simple::custom(
                            num_start..num_end,
                            "AgentScript version must be at least 1",
                        ));
                    }
                }
            }
            i = j;
            if i < b.len() && b[i] == b'\r' {
                i += 1;
            }
            if i < b.len() && b[i] == b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < b.len() && c == b'/' && b[i + 1] == b'*' {
            i += 2;
            let mut closed = false;
            while i + 1 < b.len() {
                if b[i] == b'*' && b[i + 1] == b'/' {
                    i += 2;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                break;
            }
            continue;
        }
        break;
    }
    None
}
