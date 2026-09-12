//! Tier 4 tests for E2-S5's cache commands: real process, real filesystem.
//!
//! They live in this crate for the reason `authoring.rs` records —
//! `CARGO_BIN_EXE_study-tts` is set only for the package that declares the
//! binary.

use std::{
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

fn study_tts(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_study-tts"))
        .args(arguments)
        .output()
        .expect("run the study-tts binary")
}

/// Every byte under a directory, as a comparable value.
///
/// Name and content of every entry, sorted, so a deleted file is as visible as
/// a changed one. The walking skeleton's `digest_directory` guards package
/// directories the same way; this compares the bytes themselves rather than
/// hashing them, because the fixture is four small files and a hash would cost
/// this crate a dependency to say the same thing.
fn tree_contents(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("read a cache directory") {
            let entry = entry.expect("read a cache entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("an entry is beneath the root")
                    .display()
                    .to_string();
                let bytes = std::fs::read(&path).expect("read a cache file");
                entries.push((relative, bytes));
            }
        }
    }
    entries.sort();
    entries
}

/// A prune that was not asked to delete deletes nothing.
///
/// E2-S5 task 6: "Make cache prune dry-run by default and require explicit
/// destructive confirmation." `AGENTS.md` §Ask first puts deleting a cache
/// entry behind the operator, and a default that deletes would put it behind a
/// typo instead.
///
/// The wrong implementation this rejects is the one every prune command is
/// first written as: delete now, offer `--dry-run` to those who think to ask.
/// The assertion is on the bytes rather than on a count, because a prune that
/// removed an entry and rebuilt an empty directory in its place would pass a
/// count.
#[test]
fn t4_e2_prune_dry_run_mutates_nothing() {
    let workspace = TempDir::new().expect("create a workspace");
    let cache = workspace.path().join("cache");

    // Two entries in the layout `cache::published_entries` actually walks:
    // `cache/segments/<first two characters>/<64-hex key>`. The first attempt
    // at this fixture used `cache/<shard>/<short key>`, which the walker never
    // sees, so the assertion below held for a command that could not have
    // deleted anything. A fixture the code under test does not recognize
    // proves nothing about it.
    let keys = ["a".repeat(64), "b".repeat(64)];
    for key in &keys {
        let entry = cache.join("segments").join(&key[..2]).join(key);
        std::fs::create_dir_all(&entry).expect("create a cache entry");
        std::fs::write(entry.join("segment.wav"), b"not audio").expect("write an artifact");
        std::fs::write(entry.join("entry.json"), b"{}").expect("write a record");
    }

    let before = tree_contents(&cache);
    assert!(!before.is_empty(), "the fixture wrote something to prune");

    let pruned = study_tts(&[
        "cache",
        "prune",
        "--workspace",
        &workspace.path().display().to_string(),
    ]);

    assert_eq!(
        pruned.status.code(),
        Some(0),
        "a prune that reports candidates is not a failure: {}",
        String::from_utf8_lossy(&pruned.stderr)
    );
    // The command found them. Without this the byte-comparison below would
    // hold for a prune that saw an empty cache, which is the vacuous pass this
    // test was written wrong once already.
    let reported = String::from_utf8(pruned.stdout.clone()).expect("stdout is UTF-8");
    for key in &keys {
        assert!(
            reported.contains(key.as_str()),
            "the prune reports the unreferenced entry `{key}` it declined to delete: {reported}"
        );
    }

    assert_eq!(
        tree_contents(&cache),
        before,
        "a prune nobody asked to delete left the cache byte-identical"
    );
}
