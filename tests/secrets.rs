mod common;

use common::{Env, FAKE_KEY};
use std::fs;

fn with_key(env: &Env) {
    env.ok(&["key", "generate"]);
}

#[test]
fn plaintext_private_keys_are_refused_and_nothing_is_written() {
    let env = Env::initialized();
    env.write("etc/tls/site.conf", "ok\n");
    env.write("etc/tls/privkey.pem", FAKE_KEY);
    let out = env.code(
        &["add", &env.arg("etc/tls"), "-c", "tls", "--create-category"],
        1,
    );
    assert!(out.contains("PEM private key"), "{}", out);
    assert!(!env.repo().join("tls").exists());
    let categories = fs::read_to_string(env.repo().join(".confect/categories.toml")).unwrap();
    assert!(!categories.contains("tls"), "{}", categories);
}

#[test]
fn a_secret_appearing_later_blocks_sync() {
    let env = Env::initialized();
    env.write("etc/app/app.conf", "ok\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    env.write(
        "etc/app/userlist.txt",
        "\"pgbouncer\" \"SCRAM-SHA-256$4096:c2FsdA==$a2V5:c2VydmVy\"\n",
    );
    let before = env.head_count();
    let out = env.code(&["sync"], 1);
    assert!(out.contains("SCRAM password verifier"), "{}", out);
    assert_eq!(env.head_count(), before);
    let tracked = env.git(&env.repo(), &["ls-files"]);
    assert!(!tracked.contains("userlist.txt"), "{}", tracked);
}

#[test]
fn encrypted_files_are_stored_as_age_and_restored_as_plaintext() {
    let env = Env::initialized();
    with_key(&env);
    let key = env.write("etc/tls/privkey.pem", FAKE_KEY);
    env.chmod(&key, 0o600);
    env.write("etc/tls/site.conf", "ok\n");
    env.ok(&[
        "category",
        "create",
        "tls",
        "--path",
        &env.arg("etc/tls"),
        "--encrypt",
        "*.pem",
    ]);
    env.ok(&["sync"]);

    let stored = env
        .repo()
        .join("tls")
        .join(format!("{}.age", key.strip_prefix("/").unwrap().display()));
    let ciphertext = fs::read(&stored).unwrap();
    assert!(ciphertext.starts_with(b"age-encryption.org/"));
    assert!(!env.stored("tls", &key).exists());
    let history = env.git(&env.repo(), &["log", "-p", "--all"]);
    assert!(!history.contains("PRIVATE KEY"), "plaintext in history");

    env.code(&["status", "--exit-code"], 0);
    fs::write(&key, "tampered").unwrap();
    let out = env.ok(&["diff"]);
    assert!(out.contains("-MIIEvQIBADANBgkqhkiG9w0BAQEFAASC"), "{}", out);

    env.ok(&["restore", key.to_str().unwrap(), "--yes"]);
    assert_eq!(fs::read_to_string(&key).unwrap(), FAKE_KEY);
    assert_eq!(common::mode(&key), 0o600);
}

#[test]
fn encrypting_an_already_stored_file_replaces_the_plain_copy() {
    let env = Env::initialized();
    with_key(&env);
    let token = env.write("etc/app/token", "not-a-detected-secret\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);
    assert!(env.stored("app", &token).exists());

    env.ok(&["category", "encrypt", "add", "app", token.to_str().unwrap()]);
    env.ok(&["sync"]);
    assert!(!env.stored("app", &token).exists());
    let tracked = env.git(&env.repo(), &["ls-files"]);
    assert!(tracked.contains("token.age"), "{}", tracked);
}

#[test]
fn encryption_without_recipients_is_a_clear_error() {
    let env = Env::initialized();
    env.write("etc/app/secret", "x\n");
    let out = env.code(
        &[
            "add",
            &env.arg("etc/app"),
            "-c",
            "app",
            "--create-category",
            "--encrypt",
        ],
        1,
    );
    assert!(out.contains("No age recipients"), "{}", out);
}

#[test]
fn allow_plaintext_lets_a_known_file_through() {
    let env = Env::initialized();
    let sample = env.write("etc/app/test-key.pem", FAKE_KEY);
    env.write("etc/app/app.conf", "ok\n");
    env.code(
        &["add", &env.arg("etc/app"), "-c", "app", "--create-category"],
        1,
    );
    env.ok(&[
        "category",
        "create",
        "app",
        "--path",
        &env.arg("etc/app/app.conf"),
    ]);
    env.ok(&[
        "category",
        "allow-plaintext",
        "add",
        "app",
        sample.to_str().unwrap(),
    ]);
    env.ok(&["category", "add-path", "app", sample.to_str().unwrap()]);
    env.ok(&["sync"]);
    assert!(env.stored("app", &sample).exists());
}

#[test]
fn audit_finds_plaintext_secrets_in_files_and_history() {
    let env = Env::initialized();
    let sample = env.write("etc/app/test-key.pem", FAKE_KEY);
    env.ok(&[
        "category",
        "create",
        "app",
        "--path",
        &env.arg("etc/app"),
        "--allow-plaintext",
        sample.to_str().unwrap(),
    ]);
    env.ok(&["sync"]);

    let out = env.code(&["audit"], 0);
    assert!(out.contains("Allowed in plaintext on purpose"), "{}", out);
    assert!(out.contains("test-key.pem"), "{}", out);

    env.ok(&[
        "category",
        "allow-plaintext",
        "remove",
        "app",
        sample.to_str().unwrap(),
    ]);
    let out = env.code(&["audit"], 3);
    assert!(out.contains("test-key.pem"), "{}", out);

    env.ok(&["remove", sample.to_str().unwrap()]);
    env.ok(&["sync"]);
    env.code(&["audit"], 0);
    let out = env.code(&["audit", "--history"], 3);
    assert!(out.contains("test-key.pem"), "{}", out);
    assert!(out.contains("commit"), "{}", out);
}

#[test]
fn key_generate_refuses_to_overwrite() {
    let env = Env::new();
    env.ok(&["key", "generate"]);
    let identity = env.config_dir().join("age-identity.txt");
    assert_eq!(common::mode(&identity), 0o600);
    let out = env.ok(&["key", "show"]);
    assert!(out.contains("age1"), "{}", out);
    env.code(&["key", "generate"], 1);
}

#[test]
fn binary_padding_large_files_and_key_stores_do_not_bypass_the_guard() {
    let env = Env::initialized();
    env.write("etc/app/app.conf", "ok\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);

    let padded = env.sys_path("etc/app/padded.key");
    let mut content = vec![0u8, 1, 2];
    content.extend_from_slice(FAKE_KEY.as_bytes());
    fs::write(&padded, &content).unwrap();
    let out = env.code(&["sync"], 1);
    assert!(out.contains("padded.key"), "{}", out);
    fs::remove_file(&padded).unwrap();

    let big = env.sys_path("etc/app/big.dat");
    let mut content = vec![b'x'; 9 * 1024 * 1024];
    content.extend_from_slice(FAKE_KEY.as_bytes());
    fs::write(&big, &content).unwrap();
    let out = env.code(&["sync"], 1);
    assert!(out.contains("big.dat"), "{}", out);
    fs::remove_file(&big).unwrap();

    fs::write(env.sys_path("etc/app/server.p12"), b"\x30\x82binary").unwrap();
    let out = env.code(&["sync"], 1);
    assert!(out.contains("PKCS#12"), "{}", out);
}

#[test]
fn audit_history_sees_binary_files() {
    let env = Env::initialized();
    let padded = env.sys_path("etc/app/padded.key");
    fs::create_dir_all(padded.parent().unwrap()).unwrap();
    let mut content = vec![0u8, 1, 2];
    content.extend_from_slice(FAKE_KEY.as_bytes());
    fs::write(&padded, &content).unwrap();
    env.ok(&[
        "category",
        "create",
        "app",
        "--path",
        &env.arg("etc/app"),
        "--allow-plaintext",
        padded.to_str().unwrap(),
    ]);
    env.ok(&["sync"]);
    env.ok(&[
        "category",
        "allow-plaintext",
        "remove",
        "app",
        padded.to_str().unwrap(),
    ]);
    env.ok(&["remove", padded.to_str().unwrap()]);
    env.ok(&["sync"]);
    let out = env.code(&["audit", "--history"], 3);
    assert!(out.contains("padded.key"), "{}", out);
}

#[test]
fn an_encrypted_file_and_a_plain_age_file_cannot_share_a_stored_path() {
    let env = Env::initialized();
    with_key(&env);
    let token = env.write("etc/app/token", "value\n");
    let sidecar = env.write("etc/app/token.age", "handmade sidecar\n");
    env.ok(&["add", &env.arg("etc/app"), "-c", "app", "--create-category"]);
    env.ok(&["sync"]);

    let out = env.code(
        &["category", "encrypt", "add", "app", token.to_str().unwrap()],
        1,
    );
    assert!(out.contains("collide"), "{}", out);
    assert_eq!(
        fs::read_to_string(env.stored("app", &sidecar)).unwrap(),
        "handmade sidecar\n"
    );
    env.ok(&["sync"]);
    let before = env.head_count();
    env.ok(&["sync"]);
    assert_eq!(env.head_count(), before);
}

#[test]
fn a_stored_copy_that_cannot_be_decrypted_is_kept_until_reencrypt() {
    let env = Env::initialized();
    with_key(&env);
    let key = env.write("etc/tls/privkey.pem", FAKE_KEY);
    env.ok(&[
        "category",
        "create",
        "tls",
        "--path",
        &env.arg("etc/tls"),
        "--encrypt",
        "*.pem",
    ]);
    env.ok(&["sync"]);

    let stored = env
        .repo()
        .join("tls")
        .join(format!("{}.age", key.strip_prefix("/").unwrap().display()));
    let mut damaged = fs::read(&stored).unwrap();
    damaged.truncate(damaged.len() / 2);
    fs::write(&stored, &damaged).unwrap();

    let out = env.ok(&["sync"]);
    assert!(out.contains("cannot be decrypted"), "{}", out);
    assert_eq!(fs::read(&stored).unwrap(), damaged);

    env.ok(&["sync", "--reencrypt"]);
    assert_ne!(fs::read(&stored).unwrap(), damaged);
    env.code(&["status", "--exit-code"], 0);
}
