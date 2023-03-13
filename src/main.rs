use structopt::StructOpt;
use std::string::String;
use std::fs::File;
use std::io::{BufReader, BufRead, Write};
use glob::glob;

use crate::summary_entry::SummaryEntry;
use itertools::Itertools;

mod summary_entry;


// For a given path (*.md), returns a Summary entry
// `title_from_name` - true : get title from file name, if it is README.pdf, gets the dir name.
// `title_from_name` - false : get title from 1st "^# " line or `trim_str`.
fn find_content(path: &std::path::PathBuf, base_path: &std::path::PathBuf, trim_str: &std::string::String, title_from_name:bool) -> Option<SummaryEntry> {
    let actual_title;

    if title_from_name {
        let t = path.file_stem().unwrap().to_str().unwrap().to_string();
        if t == String::from("README") {
            let path_str = path.to_str().expect("");
            let mut spl = path_str.split('/');
            spl.next_back();
            actual_title = String::from(spl.next_back().expect("split error"));
        }
        else {
            actual_title = path.file_stem().unwrap().to_str().unwrap().to_string();
        }
        return Some(SummaryEntry {
            path: relative_path(path, base_path),
            title: actual_title,
        });
    }
    // else get title from inside the file
    File::open(path).map(|f| {
        BufReader::new(f).lines()
            .filter_map(|l| l.ok())
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with(trim_str) {
                    Some(trimmed.trim_start_matches(trim_str).to_string())
                } else {
                    None
                }
            })
            .next().map(|title| {
            SummaryEntry {
                path: relative_path(path, base_path),
                title,
            }
        })
    }).unwrap_or(None)
}

// Build the vector of Summary entries for a given dir
fn handle_dir(path: &std::path::PathBuf,
                base_path: &std::path::PathBuf,
                trim_str: &std::string::String,
                title_from_name:bool,
                sum_entries: &mut Vec<SummaryEntry>) {
    // Handle md files in directory
    let glob_string = String::from(path.to_str().expect("given path should be a string")) + "/*.md";
    let entries = glob(&glob_string).expect("Failed to read glob pattern")
        .filter_map(|e| e.ok())
        .filter_map(|e| find_content(&e, &base_path, &trim_str, title_from_name));
    for sum_line in entries {
        sum_entries.push(sum_line);
    }

    // Handle missing README.md (to get a decent tree in SUMMARY.md)
    let glob_string = String::from(path.to_str().expect("given path should be a string")) + "/README.md";
    let readme = glob(&glob_string).expect("Failed to read glob pattern").filter_map(|e| e.ok());

    // No README - push a Summary entry to the vector
    if readme.count() == 0 {
        let mut readme_path = relative_path(path, base_path);
        readme_path.push("README.md");
        sum_entries.push (SummaryEntry{
                            path: readme_path,
                            title: String::from(relative_path(path, base_path).to_str().expect("")
                                .split("/").last().expect("split error")),
                            });
    }
}

// Builds a dir/file's path relative to `base_path`.
fn relative_path(path: &std::path::PathBuf, base_path: &std::path::PathBuf) -> std::path::PathBuf {
    path.strip_prefix(base_path)
        .expect("Given a base path that is not actually a base path for current directory")
        .to_path_buf()
}

// Write a line to SUMMARY.md
fn write_line(file: &mut File, line: &str, verbose: bool) {
    writeln!(file, "{}", line).expect("Unable to write to file");
    if verbose {
        println!("{}", line);
    }
}

// Options management
#[derive(StructOpt)]
#[structopt(about = "Program to produce a mdbook SUMMARY.md file from doc tree. `-h` for help.")]
struct Cli {
    #[structopt(parse(from_os_str), default_value="src/", help="MD src documentation root")]
    base_path: std::path::PathBuf,
    #[structopt(short = "v", long = "verbose", help="Verbose mode")]
    verbose: bool,
    #[structopt(long = "trim_str", default_value="# ", help="Trim string to retrieve title from a file")]
    trim_str: std::string::String,
    #[structopt(long = "title_from_name", help="Get titles from MD file names")]
    title_from_name: bool,
    #[structopt(long = "create_readmes", help="Create README.md in dirs when not already present")]
    create_readmes: bool,
}

fn main() {
    // Options management
    let args = Cli::from_args();

    let base_path = args.base_path;
    let verbose = args.verbose;
    let trim_str = args.trim_str;
    let title_from_name = args.title_from_name;
    let create_readmes = args.create_readmes;

    let mut sum_entries: Vec<SummaryEntry>;

    if create_readmes {
        sum_entries = vec![ SummaryEntry{
            path: std::path::PathBuf::from("README.md"),
            title: String::from("README")
        } ];

        let glob_string = String::from(base_path.to_str().expect("given path should be a string")) + "/**/";
        let entries = glob(&glob_string)
        .expect("Failed to read glob pattern")
        .filter_map(|e| e.ok());

        for path in entries {
            handle_dir(&path, &base_path, &trim_str, title_from_name, &mut sum_entries)
        }
    }
    else {
        // Fallback to original code
        // Find all *.md files in tree
        let glob_string = String::from(base_path.to_str().expect("given path should be a string")) + "/**/*.md";
        // Retrieve all summary entries, title and path
        sum_entries = glob(&glob_string)
            .expect("Failed to read glob pattern")
            .filter_map(|e| e.ok())
            .filter_map(|e| find_content(&e, &base_path, &trim_str, title_from_name))
            .collect();
    }

    // Dump entries to file
    let mut summary = File::create(base_path.join("SUMMARY.md"))
        .expect("Failed to create SUMMARY.md");

    write_line(&mut summary, "# https://github.com/rust-lang-nursery/mdBook/issues/677", verbose);

    // From SummaryEntry vector, sort and build SUMMARY lines
    let entry_lines = sum_entries.into_iter().sorted().map(|e| e.summary_line());

    // Dump lines to file
    for el in entry_lines {
        write_line(&mut summary, format!("{}", el).as_str(), verbose);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn find_content_finds_a_title() {
        let mut file = NamedTempFile::new().expect("should have created a tempfile");
        writeln!(file, "\n\n# A File").expect("should have written to file");

        let entry = find_content(&file.path().to_path_buf(), &file.path().parent().unwrap().to_path_buf(), &String::from("# "), false);

        assert_eq!(entry.is_none(), false);

        if let Some(ref entry) = entry {
            assert_eq!(entry.title, "A File".to_string());
        }
    }

    #[test]
    fn find_content_returns_no_title_if_none_found() {
        let mut file = NamedTempFile::new().expect("should have created a tempfile");
        writeln!(file, "\n\n").expect("should have written to file");

        let entry = find_content(&file.path().to_path_buf(), &file.path().parent().unwrap().to_path_buf(), &String::from("# "), false);

        assert_eq!(entry.is_none(), true);
    }
}
