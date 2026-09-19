mod common;

use common::{Env, HOST};
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn with_remote(env: &Env) -> std::path::PathBuf {
    let remote = env.bare_remote("remote.git");
    env.ok(&["init", "--host", HOST, "--remote", remote.to_str().unwrap()]);
    env.write("etc/app/app.conf", "v1\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    remote
}

#[test]
fn sync_pushes_the_host_branch() {
    let env = Env::new();
    let remote = with_remote(&env);
    let out = env.ok(&["sync"]);
    assert!(out.contains("Pushed host/test-host"), "{}", out);
    let branches = env.git(&remote, &["branch", "--list"]);
    assert!(branches.contains("host/test-host"), "{}", branches);
    let out = env.ok(&["sync"]);
    assert!(out.contains("up to date"), "{}", out);
}

#[test]
fn auto_push_false_and_no_push_are_respected() {
    let env = Env::new();
    let remote = with_remote(&env);
    let config = env.config_dir().join("config.toml");
    let content = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        content.replace("auto_push = true", "auto_push = false"),
    )
    .unwrap();
    env.ok(&["sync"]);
    assert!(env.git(&remote, &["branch", "--list"]).trim().is_empty());
    env.ok(&["push"]);
    assert!(!env.git(&remote, &["branch", "--list"]).trim().is_empty());
}

#[test]
fn a_rejected_push_is_an_error() {
    let env = Env::new();
    let remote = with_remote(&env);
    let hook = remote.join("hooks/pre-receive");
    fs::write(&hook, "#!/bin/sh\necho denied by policy >&2\nexit 1\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();

    let out = env.code(&["sync"], 1);
    assert!(out.contains("push failed"), "{}", out);
    assert!(out.contains("denied by policy"), "{}", out);
}

#[test]
fn init_from_continues_the_host_branch_and_restores_it() {
    let env = Env::new();
    let remote = with_remote(&env);
    let file = env.sys_path("etc/app/app.conf");
    env.ok(&["sync"]);

    // A rebuilt machine: fresh home, no system files.
    let rebuilt = Env::new();
    fs::remove_file(&file).unwrap();
    let out = rebuilt.ok(&["init", "--from", remote.to_str().unwrap(), "--host", HOST]);
    assert!(out.contains("Checked out branch host/test-host"), "{}", out);
    assert_eq!(common::mode(&rebuilt.repo()), 0o700);

    rebuilt.ok(&["restore", "--yes"]);
    assert_eq!(fs::read_to_string(&file).unwrap(), "v1\n");
}

#[test]
fn init_from_starts_an_empty_branch_for_a_new_host() {
    let env = Env::new();
    let remote = with_remote(&env);
    env.ok(&["sync"]);

    let other = Env::new();
    let out = other.ok(&[
        "init",
        "--from",
        remote.to_str().unwrap(),
        "--host",
        "second",
    ]);
    assert!(out.contains("started it empty"), "{}", out);
    let tracked = other.git(&other.repo(), &["ls-files"]);
    assert!(!tracked.contains("app.conf"), "{}", tracked);
    other.ok(&["sync"]);
    let branches = env.git(&remote, &["branch", "--list"]);
    assert!(branches.contains("host/second"), "{}", branches);
    assert!(branches.contains("host/test-host"), "{}", branches);
}

#[test]
fn pull_fast_forwards_only_this_hosts_branch() {
    let env = Env::new();
    let remote = with_remote(&env);
    env.ok(&["sync"]);

    // Another checkout of the same host branch pushes a change.
    let other = Env::new();
    other.ok(&["init", "--from", remote.to_str().unwrap(), "--host", HOST]);
    fs::write(env.sys_path("etc/app/app.conf"), "v2\n").unwrap();
    other.ok(&["sync"]);
    fs::write(env.sys_path("etc/app/app.conf"), "v1\n").unwrap();

    // A different host pushes too; its branch must not leak into ours.
    let stranger = Env::new();
    stranger.ok(&["init", "--from", remote.to_str().unwrap(), "--host", "aaa"]);
    stranger.ok(&["sync"]);

    // add + sync leave nothing uncommitted, so the dirty-tree check lets pull through.
    assert!(env
        .git(&env.repo(), &["status", "--porcelain"])
        .trim()
        .is_empty());
    let out = env.ok(&["pull"]);
    assert!(out.contains("Fast-forwarded"), "{}", out);
    let stored = fs::read_to_string(env.stored("app", &env.sys_path("etc/app/app.conf"))).unwrap();
    assert_eq!(stored, "v2\n");
}

#[test]
fn diverged_branches_are_reported_not_merged() {
    let env = Env::new();
    let remote = with_remote(&env);
    env.ok(&["sync"]);

    let other = Env::new();
    other.ok(&["init", "--from", remote.to_str().unwrap(), "--host", HOST]);
    fs::write(env.sys_path("etc/app/app.conf"), "remote change\n").unwrap();
    other.ok(&["sync"]);

    fs::write(env.sys_path("etc/app/app.conf"), "local change\n").unwrap();
    env.ok(&["sync", "--no-push"]);
    let out = env.code(&["pull"], 1);
    assert!(out.contains("diverged"), "{}", out);
}

#[test]
fn pull_refuses_to_run_over_uncommitted_changes() {
    let env = Env::new();
    with_remote(&env);
    let out = env.code(&["pull"], 1);
    assert!(out.contains("run 'confect sync' first"), "{}", out);
}

#[test]
fn option_like_urls_are_rejected() {
    let env = Env::new();
    let out = env.code(
        &["init", "--host", HOST, "--from=--upload-pack=touch /tmp/x"],
        1,
    );
    assert!(out.contains("is not a repository URL"), "{}", out);
    assert!(!env.repo().exists());
}
