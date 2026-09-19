mod common;

use common::{is_root, mode, Env, HOST};
use std::fs;
use std::os::unix::ffi::OsStrExt;

#[test]
fn init_creates_a_private_repository_on_the_host_branch() {
    let env = Env::initialized();
    assert_eq!(mode(&env.repo()), 0o700);
    let branch = env.git(&env.repo(), &["symbolic-ref", "--short", "HEAD"]);
    assert_eq!(branch.trim(), format!("host/{}", HOST));
    assert!(env.git(&env.repo(), &["branch", "--list"]).lines().count() == 1);
}

#[test]
fn init_keeps_existing_global_settings() {
    let env = Env::new();
    env.write_config("[global]\nauto_push = false\nnetwork_timeout = 42\n");
    env.ok(&["init", "--host", HOST]);
    let config = fs::read_to_string(env.config_dir().join("config.toml")).unwrap();
    assert!(config.contains("auto_push = false"), "{}", config);
    assert!(config.contains("network_timeout = 42"), "{}", config);
}

#[test]
fn init_with_path_remembers_the_repository() {
    let env = Env::new();
    let custom = env.root.path().join("elsewhere");
    env.ok(&["init", "--host", HOST, "--path", custom.to_str().unwrap()]);
    let out = env.ok(&["info"]);
    assert!(out.contains(custom.to_str().unwrap()), "{}", out);
}

#[test]
fn init_rejects_invalid_host_without_leaving_anything_behind() {
    let env = Env::new();
    let out = env.code(&["init", "--host", "bad host"], 1);
    assert!(out.contains("Invalid host name"), "{}", out);
    assert!(!env.repo().exists());
}

#[test]
fn repo_option_and_environment_select_the_repository() {
    let env = Env::new();
    let custom = env.root.path().join("explicit");
    env.ok(&["--repo", custom.to_str().unwrap(), "init", "--host", HOST]);
    env.ok(&["--repo", custom.to_str().unwrap(), "info"]);
    let output = env
        .cmd()
        .env("CONFECT_REPO", &custom)
        .arg("info")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn unknown_config_keys_are_reported() {
    let env = Env::new();
    env.write_config("[repository]\npath = \"/somewhere\"\n");
    let out = env.code(&["info"], 1);
    assert!(out.contains("[global] repo_path"), "{}", out);
}

#[test]
fn add_and_sync_commit_files_with_metadata() {
    let env = Env::initialized();
    let file = env.write("etc/app/app.conf", "a = 1\n");
    env.chmod(&file, 0o640);
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    let out = env.ok(&["sync"]);
    assert!(out.contains("Sync app (A1)"), "{}", out);
    assert_eq!(
        fs::read_to_string(env.stored("app", &file)).unwrap(),
        "a = 1\n"
    );

    let metadata = fs::read_to_string(env.repo().join(".confect/metadata.toml")).unwrap();
    assert!(metadata.contains("mode = \"0640\""), "{}", metadata);
    assert!(metadata.contains("kind = \"dir\""), "{}", metadata);

    let out = env.ok(&["status"]);
    assert!(out.contains("matches the repository"), "{}", out);
}

#[test]
fn sync_without_changes_succeeds_and_makes_no_commit() {
    let env = Env::initialized();
    env.write("etc/app/app.conf", "x\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    let before = env.head_count();
    let out = env.ok(&["sync"]);
    assert!(out.contains("Nothing to commit"), "{}", out);
    assert_eq!(env.head_count(), before);
}

#[test]
fn status_reports_new_modified_deleted_and_permission_changes() {
    let env = Env::initialized();
    let keep = env.write("etc/app/keep.conf", "keep\n");
    let edit = env.write("etc/app/edit.conf", "one\n");
    let gone = env.write("etc/app/gone.conf", "bye\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);

    fs::write(&edit, "two\n").unwrap();
    fs::remove_file(&gone).unwrap();
    env.write("etc/app/new.conf", "new\n");
    env.chmod(&keep, 0o600);

    let out = env.code(&["status", "--exit-code"], 3);
    assert!(out.contains(&format!("M {}", edit.display())), "{}", out);
    assert!(out.contains(&format!("D {}", gone.display())), "{}", out);
    assert!(
        out.contains(&format!("A {}", env.sys_path("etc/app/new.conf").display())),
        "{}",
        out
    );
    assert!(out.contains(&format!("P {}", keep.display())), "{}", out);
    assert!(out.contains("mode 0644 → 0600"), "{}", out);

    env.ok(&["sync"]);
    env.code(&["status", "--exit-code"], 0);
}

#[test]
fn diff_is_a_real_unified_diff() {
    let env = Env::initialized();
    let file = env.write("etc/app/app.conf", "a\nb\nc\nd\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    fs::write(&file, "a\ninserted\nb\nc\nd\n").unwrap();

    let out = env.ok(&["diff"]);
    assert!(out.contains("--- repo:"), "{}", out);
    assert!(out.contains("+++ system:"), "{}", out);
    assert!(out.contains("+inserted"), "{}", out);
    assert!(!out.contains("-b"), "positional diff: {}", out);
}

#[test]
fn diff_marks_binary_files() {
    let env = Env::initialized();
    let file = env.sys_path("etc/app/blob.bin");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, b"\x00\x01\x02").unwrap();
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    fs::write(&file, b"\x00\x01\x03").unwrap();
    let out = env.ok(&["diff"]);
    assert!(out.contains("Binary files"), "{}", out);
}

#[test]
fn excluded_files_and_directories_never_reach_the_repository() {
    let env = Env::initialized();
    env.write("etc/app/app.conf", "ok\n");
    let secret = env.write("etc/app/local.override", "skip me\n");
    let cache = env.write("etc/app/cache/state.db", "cache\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&[
        "category",
        "exclude",
        "add",
        "app",
        &secret.to_string_lossy(),
    ]);
    env.ok(&["category", "exclude", "add", "app", "cache"]);
    env.ok(&["sync"]);

    assert!(!env.stored("app", &secret).exists());
    assert!(!env.stored("app", &cache).exists());
    assert!(env
        .stored("app", &env.sys_path("etc/app/app.conf"))
        .exists());
}

#[test]
fn excluding_later_drops_already_stored_copies() {
    let env = Env::initialized();
    let noisy = env.write("etc/app/noisy.log", "log\n");
    env.write("etc/app/app.conf", "ok\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    assert!(env.stored("app", &noisy).exists());

    env.ok(&["category", "exclude", "add", "app", "*.log"]);
    assert!(!env.stored("app", &noisy).exists());
    env.ok(&["sync"]);
    let tracked = env.git(&env.repo(), &["ls-files"]);
    assert!(!tracked.contains("noisy.log"), "{}", tracked);
}

#[test]
fn nested_git_directories_are_skipped() {
    let env = Env::initialized();
    env.write("etc/app/app.conf", "ok\n");
    env.git(&env.sys_path("etc/app"), &["init", "-q"]);
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    let tracked = env.git(&env.repo(), &["ls-files"]);
    assert!(!tracked.contains("/.git/"), "{}", tracked);
    env.ok(&["sync"]);
}

#[test]
fn a_tracked_gitignore_does_not_hide_files() {
    let env = Env::initialized();
    env.write("etc/app/.gitignore", "*.conf\n");
    let conf = env.write("etc/app/app.conf", "ok\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    let tracked = env.git(&env.repo(), &["ls-files"]);
    assert!(tracked.contains("app.conf"), "{}", tracked);
    assert!(env.stored("app", &conf).exists());
}

#[test]
fn symlinks_are_tracked_as_links_not_targets() {
    let env = Env::initialized();
    let target = env.write("etc/app/real.conf", "real\n");
    let link = env.sys_path("etc/app/link.conf");
    std::os::unix::fs::symlink("real.conf", &link).unwrap();
    env.ok(&[
        "add",
        link.to_str().unwrap(),
        "-c",
        "links",
        "--create-category",
    ]);
    env.ok(&["sync"]);

    let stored = env.stored("links", &link);
    assert!(fs::symlink_metadata(&stored)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!env.stored("links", &target).exists());
}

#[test]
fn dangling_symlinks_and_odd_names_do_not_cause_endless_commits() {
    let env = Env::initialized();
    env.write("etc/app/app.conf", "ok\n");
    std::os::unix::fs::symlink("/nonexistent/target", env.sys_path("etc/app/dangling")).unwrap();
    let odd = env
        .sys_path("etc/app")
        .join(std::ffi::OsStr::from_bytes(b"bad-\xff-name"));
    fs::write(&odd, "x").unwrap();
    nix::unistd::mkfifo(&env.sys_path("etc/app/pipe"), nix::sys::stat::Mode::S_IRWXU).unwrap();

    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    let out = env.ok(&["sync"]);
    assert!(out.contains("not valid UTF-8"), "{}", out);
    let before = env.head_count();
    let out = env.ok(&["sync"]);
    assert!(out.contains("Nothing to commit"), "{}", out);
    assert_eq!(env.head_count(), before);
}

#[test]
fn a_missing_tracked_root_keeps_its_copies() {
    let env = Env::initialized();
    let file = env.write("etc/app/app.conf", "ok\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);

    fs::remove_dir_all(env.sys_path("etc/app")).unwrap();
    let out = env.ok(&["sync"]);
    assert!(out.contains("is missing"), "{}", out);
    assert!(env.stored("app", &file).exists());

    env.ok(&["remove", &env.arg("etc/app")]);
    env.ok(&["sync"]);
    assert!(!env.stored("app", &file).exists());
}

#[test]
fn remove_inside_a_tracked_directory_excludes_and_drops_the_copy() {
    let env = Env::initialized();
    let drop = env.write("etc/app/drop.conf", "x\n");
    env.write("etc/app/keep.conf", "y\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);

    env.ok(&["remove", drop.to_str().unwrap()]);
    env.ok(&["sync"]);
    assert!(!env.stored("app", &drop).exists());
    let out = env.ok(&["category", "show", "app"]);
    assert!(out.contains("Excluded"), "{}", out);
    env.code(&["status", "--exit-code"], 0);
}

#[test]
fn adding_twice_or_overlapping_is_rejected() {
    let env = Env::initialized();
    let file = env.write("etc/app/app.conf", "x\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    let out = env.code(&["add", file.to_str().unwrap(), "-c", "app"], 1);
    assert!(out.contains("already tracked"), "{}", out);
    let out = env.code(
        &["add", &env.arg("etc"), "-c", "other", "--create-category"],
        1,
    );
    assert!(out.contains("overlaps"), "{}", out);
}

#[test]
fn add_requires_an_existing_category_unless_asked_to_create_it() {
    let env = Env::initialized();
    env.write("etc/app/app.conf", "x\n");
    let out = env.code(&["add", &env.arg("etc/app"), "-c", "nope"], 1);
    assert!(out.contains("--create-category"), "{}", out);
    let tracked = env.git(&env.repo(), &["status", "--porcelain"]);
    assert!(tracked.trim().is_empty(), "{}", tracked);
}

#[test]
fn deleting_a_category_drops_its_copies() {
    let env = Env::initialized();
    let file = env.write("etc/app/app.conf", "x\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    env.ok(&["category", "delete", "app", "--yes"]);
    env.ok(&["sync"]);
    assert!(!env.stored("app", &file).exists());
    assert!(!env.repo().join("app").exists());
}

#[test]
fn unreadable_files_keep_their_stored_copy() {
    if is_root() {
        return;
    }
    let env = Env::initialized();
    let file = env.write("etc/app/private.conf", "v1\n");
    env.write("etc/app/app.conf", "ok\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);

    env.chmod(&file, 0o000);
    let out = env.ok(&["sync"]);
    env.chmod(&file, 0o644);
    assert!(out.contains("cannot read"), "{}", out);
    assert_eq!(
        fs::read_to_string(env.stored("app", &file)).unwrap(),
        "v1\n"
    );
}

#[test]
fn category_files_are_written_in_a_stable_order() {
    let env = Env::initialized();
    for name in ["zeta", "alpha", "mid"] {
        env.write(&format!("etc/{}/x.conf", name), "x\n");
        env.ok(&[
            "add",
            &env.arg(&format!("etc/{}", name)),
            "-c",
            name,
            "--create-category",
        ]);
    }
    let first = fs::read_to_string(env.repo().join(".confect/categories.toml")).unwrap();
    env.ok(&["category", "exclude", "add", "mid", "*.tmp"]);
    env.ok(&["category", "exclude", "remove", "mid", "*.tmp"]);
    let second = fs::read_to_string(env.repo().join(".confect/categories.toml")).unwrap();
    assert_eq!(first, second);
    let alpha = first.find("[categories.alpha]").unwrap();
    let zeta = first.find("[categories.zeta]").unwrap();
    assert!(alpha < zeta);
}

#[test]
fn a_second_run_waits_for_the_repository_lock() {
    let env = Env::initialized();
    env.write("etc/app/app.conf", "x\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    let lock = env.repo().join(".git/confect.lock");
    let mut holder = std::process::Command::new("flock")
        .arg(&lock)
        .args(["sleep", "1"])
        .spawn()
        .expect("flock(1) is available");
    std::thread::sleep(std::time::Duration::from_millis(200));
    let started = std::time::Instant::now();
    let out = env.ok(&["sync"]);
    holder.wait().unwrap();
    assert!(
        out.contains("Waiting for another confect process"),
        "{}",
        out
    );
    assert!(started.elapsed() >= std::time::Duration::from_millis(500));
}

#[test]
fn a_pattern_edit_does_not_record_unrelated_changes() {
    let env = Env::initialized();
    let conf = env.write("etc/app/app.conf", "v1\n");
    env.write("etc/app/debug.log", "log\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);

    fs::write(&conf, "unreviewed edit\n").unwrap();
    env.ok(&["category", "exclude", "add", "app", "*.log"]);
    assert_eq!(
        fs::read_to_string(env.stored("app", &conf)).unwrap(),
        "v1\n"
    );
    let out = env.code(&["status", "--exit-code"], 3);
    assert!(out.contains(&format!("M {}", conf.display())), "{}", out);
}
