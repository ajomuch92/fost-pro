//! What a project is made of: the programs, forms, classes and tables of a folder.

use crate::is_kw;
use serde_json::{Value as Json, json};
use std::path::Path;

/// The classes a program defines: name, parent and the line of each DEFINE CLASS.
// shortcut: reads the text line by line, so a DEFINE CLASS split with `;` shows without its parent
fn classes(file: &str, src: &str) -> Vec<Json> {
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let kw = |n: usize, full: &str| words.get(n).is_some_and(|w| is_kw(&w.to_uppercase(), full));
        if kw(0, "DEFINE") && kw(1, "CLASS") && words.len() > 2 {
            let parent = if kw(3, "AS") { words.get(4).copied() } else { None };
            out.push(json!({ "name": words[2], "parent": parent, "file": file, "line": i + 1 }));
        }
    }
    out
}

/// Everything a project panel lists for the folder `dir`. A folder that cannot be read is an empty project.
pub fn project(dir: &Path) -> Json {
    let entries = std::fs::read_dir(dir).ok().into_iter().flatten().flatten();
    let mut files: Vec<String> = entries.map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    files.sort_by_key(|f| f.to_lowercase());
    let with = |ext: &str| files.iter().filter(|f| f.to_lowercase().ends_with(ext)).cloned().collect::<Vec<_>>();
    let programs = with(".fost");
    let defined: Vec<Json> =
        programs.iter().flat_map(|f| classes(f, &std::fs::read_to_string(dir.join(f)).unwrap_or_default())).collect();
    json!({ "programs": programs, "forms": with(".fostform"), "classes": defined, "tables": crate::data::tables(dir) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_classes_of_a_program() {
        let src = "x = 1\n  define class Ficha as Form\nENDDEFINE\nDEFI CLAS Base\n* DEFINE CLASS no\ndefine = 3";
        let found = classes("a.fost", src);
        assert_eq!(found, [
            json!({ "name": "Ficha", "parent": "Form", "file": "a.fost", "line": 2 }),
            json!({ "name": "Base", "parent": null, "file": "a.fost", "line": 4 }),
        ]);
    }
}
