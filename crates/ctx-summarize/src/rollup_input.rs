//! Assembly of a rollup prompt's user input from a directory's current
//! children summaries and its `intent.md`.

use crate::cpath;
use crate::error::SummError;
use crate::fs::Fs;

/// Display form of a directory (`""` -> `.`).
pub const fn dir_label(dir: &str) -> &str {
    if dir.is_empty() {
        "."
    } else {
        dir
    }
}

/// Whether `name` has a (case-insensitive) `.ctx` extension.
fn has_ctx_ext(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ctx"))
}

/// Append a labeled section to the rollup input buffer.
fn push_section(buf: &mut String, label: &str, body: &str) {
    buf.push_str("\n--- ");
    buf.push_str(label);
    buf.push_str(" ---\n");
    buf.push_str(body);
    if !body.ends_with('\n') {
        buf.push('\n');
    }
}

/// Assemble the rollup prompt's user input for `dir` from current
/// children summaries and the directory's `intent.md`.
pub fn assemble_rollup_input<F: Fs>(fs: &F, dir: &str) -> Result<String, SummError> {
    let mut buf = format!("DIR_PATH: {}\n", dir_label(dir));
    let intent_path = cpath::intent_of(dir);
    let intent = if fs.exists(&intent_path) {
        fs.read(&intent_path)?
    } else {
        String::new()
    };
    buf.push_str("\n--- intent.md ---\n");
    buf.push_str(if intent.is_empty() {
        "(none)\n"
    } else {
        &intent
    });
    let cdir = cpath::context_dir_of(dir);
    for name in fs.list_dir(&cdir)? {
        if name == "rollup.ctx" || name == "intent.md" {
            continue;
        }
        let child_rel = format!("{cdir}/{name}");
        if has_ctx_ext(&name) {
            push_section(&mut buf, &name, &fs.read(&child_rel)?);
        } else if fs.exists(&format!("{child_rel}/rollup.ctx")) {
            let sub = fs.read(&format!("{child_rel}/rollup.ctx"))?;
            push_section(&mut buf, &format!("{name}/rollup.ctx"), &sub);
        }
    }
    Ok(buf)
}
