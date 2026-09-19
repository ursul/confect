mod common;

use common::{mode, Env, FAKE_KEY, HOST};
use std::fs;
use std::os::unix::fs::PermissionsExt;

/// Build a repository the way confect 1.4.1 left it.
fn legacy_repo(env: &Env) -> std::path::PathBuf {
    let repo = env.repo();
    fs::create_dir_all(repo.join(".confect")).unwrap();
    fs::set_permissions(&repo, fs::Permissions::from_mode(0o755)).unwrap();
    env.git(&repo, &["init", "-q", "-b", "master"]);
    fs::write(
        repo.join(".confect/config.toml"),
        "[repository]\nversion = 1\n\n[hosts]\nstrategy = \"branch\"\n\n[hosts.list.test-host]\nbranch = \"host/test-host\"\n",
    )
    .unwrap();
    fs::write(repo.join(".confect/metadata.toml"), "[files]\n").unwrap();
    fs::write(repo.join(".gitignore"), "*.confect-backup\n").unwrap();

    let conf = env.write("etc/app/app.conf", "legacy\n");
    env.chmod(&conf, 0o640);
    let key = env.write("etc/app/server.key", FAKE_KEY);
    fs::write(
        repo.join(".confect/categories.toml"),
        format!(
            "[categories.app]\npaths = [\"{}\"]\nencrypt = []\nexclude = []\n",
            env.arg("etc/app")
        ),
    )
    .unwrap();
    for file in [&conf, &key] {
        let stored = env.stored("app", file);
        fs::create_dir_all(stored.parent().unwrap()).unwrap();
        fs::copy(file, &stored).unwrap();
    }
    env.git(&repo, &["add", "-A"]);
    env.git(
        &repo,
        &["commit", "-q", "-m", "Initialize confect repository"],
    );
    env.git(&repo, &["checkout", "-q", "-b", "host/test-host"]);
    env.write_config("[global]\ndefault_remote = \"origin\"\nauto_push = true\n\n[encryption]\nenabled = false\n\n[hosts]\nstrategy = \"branch\"\ncurrent = \"test-host\"\n");
    repo
}

#[test]
fn commands_on_a_legacy_repository_ask_for_migration() {
    let env = Env::new();
    legacy_repo(&env);
    let out = env.code(&["status"], 1);
    assert!(out.contains("confect migrate"), "{}", out);
    let out = env.ok(&["info"]);
    assert!(out.contains("format   1"), "{}", out);
}

#[test]
fn migrate_indexes_files_restricts_permissions_and_reports_secrets() {
    let env = Env::new();
    let repo = legacy_repo(&env);
    let out = env.ok(&["migrate", "--yes"]);
    assert!(out.contains("Indexed 2 stored path(s)"), "{}", out);
    assert!(out.contains("server.key"), "{}", out);
    assert_eq!(mode(&repo), 0o700);

    let config = fs::read_to_string(repo.join(".confect/config.toml")).unwrap();
    assert!(config.contains("version = 2"), "{}", config);
    assert!(
        config.contains(&format!("host = \"{}\"", HOST)),
        "{}",
        config
    );
    let metadata = fs::read_to_string(repo.join(".confect/metadata.toml")).unwrap();
    assert!(metadata.contains("mode = \"0640\""), "{}", metadata);

    let log = env.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(log.trim(), "Migrate repository to confect 2 format");
    env.ok(&["migrate"]);

    // The key is already stored in plaintext: sync does not re-add it, audit reports it.
    env.code(&["audit"], 2);
    env.ok(&["category", "exclude", "add", "app", "*.key"]);
    env.ok(&["sync", "--no-push"]);
    env.code(&["audit"], 0);
    env.code(&["audit", "--history"], 2);
}
