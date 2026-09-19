mod common;

use common::{mode, Env};
use std::fs;

fn tracked_app(env: &Env) -> std::path::PathBuf {
    let file = env.write("etc/app/app.conf", "original\n");
    env.chmod(&file, 0o640);
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    file
}

#[test]
fn restore_writes_content_and_permissions_back() {
    let env = Env::initialized();
    let file = tracked_app(&env);
    fs::write(&file, "changed\n").unwrap();
    env.chmod(&file, 0o666);

    let out = env.ok(&["restore", "--dry-run"]);
    assert!(out.contains("overwrite"), "{}", out);
    assert_eq!(fs::read_to_string(&file).unwrap(), "changed\n");

    env.ok(&["restore", "--yes"]);
    assert_eq!(fs::read_to_string(&file).unwrap(), "original\n");
    assert_eq!(mode(&file), 0o640);
    env.code(&["status", "--exit-code"], 0);
}

#[test]
fn restore_recreates_deleted_files_and_directories() {
    let env = Env::initialized();
    let file = tracked_app(&env);
    fs::remove_dir_all(env.sys_path("etc/app")).unwrap();
    env.ok(&["restore", "-c", "app", "--yes"]);
    assert_eq!(fs::read_to_string(&file).unwrap(), "original\n");
}

#[test]
fn restore_without_yes_and_without_a_terminal_refuses() {
    let env = Env::initialized();
    let file = tracked_app(&env);
    fs::write(&file, "changed\n").unwrap();
    let out = env.code(&["restore"], 1);
    assert!(out.contains("--yes"), "{}", out);
    assert_eq!(fs::read_to_string(&file).unwrap(), "changed\n");
}

#[test]
fn a_failed_restore_leaves_the_live_file_alone_and_fails() {
    let env = Env::initialized();
    let file = tracked_app(&env);
    fs::write(&file, "live\n").unwrap();
    fs::remove_file(env.stored("app", &file)).unwrap();

    env.code(&["restore", "--yes"], 1);
    assert_eq!(fs::read_to_string(&file).unwrap(), "live\n");
}

#[test]
fn restore_replaces_a_planted_symlink_instead_of_writing_through_it() {
    let env = Env::initialized();
    let file = tracked_app(&env);
    let victim = env.write("victim", "precious\n");
    fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(&victim, &file).unwrap();

    env.ok(&["restore", "--yes", "--backup"]);
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious\n");
    assert!(!fs::symlink_metadata(&file)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&file).unwrap(), "original\n");
}

#[test]
fn backups_are_kept_next_to_overwritten_files() {
    let env = Env::initialized();
    let file = tracked_app(&env);
    fs::write(&file, "local edit\n").unwrap();
    env.ok(&["restore", "--yes", "--backup"]);

    let backups: Vec<_> = fs::read_dir(file.parent().unwrap())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains(".confect-backup."))
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::read_to_string(backups[0].path()).unwrap(),
        "local edit\n"
    );
    env.ok(&["sync"]);
    let tracked = env.git(&env.repo(), &["ls-files"]);
    assert!(!tracked.contains("confect-backup"), "{}", tracked);
}

#[test]
fn restoring_an_untracked_path_is_an_error() {
    let env = Env::initialized();
    tracked_app(&env);
    let out = env.code(&["restore", &env.arg("etc/other"), "--yes"], 1);
    assert!(out.contains("not tracked"), "{}", out);
}

#[test]
fn unchanged_files_are_not_touched() {
    let env = Env::initialized();
    tracked_app(&env);
    let out = env.ok(&["restore", "--yes"]);
    assert!(out.contains("Nothing to restore"), "{}", out);
}
