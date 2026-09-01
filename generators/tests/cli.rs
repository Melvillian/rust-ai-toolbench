//! Runs the `generators` binary end-to-end in a temporary repo layout.

use std::process::Command;

#[test]
fn render_deploy_writes_dockerfile_and_render_yaml() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("apps/foo")).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_generators"))
        .current_dir(repo.path())
        .args(["render-deploy", "foo"])
        .output()
        .expect("run generators");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let repo_name = repo
        .path()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let dockerfile = std::fs::read_to_string(repo.path().join("apps/foo/Dockerfile")).unwrap();
    assert!(dockerfile.contains("--package foo"));

    let yaml = std::fs::read_to_string(repo.path().join("render.yaml")).unwrap();
    assert!(yaml.contains(&format!("name: {repo_name}-foo")));
    assert!(yaml.contains("dockerfilePath: apps/foo/Dockerfile"));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("✓ Created Dockerfile"));
    assert!(stdout.contains("✓ Updated render.yaml"));
}

#[test]
fn render_deploy_static_skips_dockerfile() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("apps/web")).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_generators"))
        .current_dir(repo.path())
        .args(["render-deploy", "web", "--static"])
        .output()
        .expect("run generators");
    assert!(out.status.success());
    assert!(!repo.path().join("apps/web/Dockerfile").exists());
    let yaml = std::fs::read_to_string(repo.path().join("render.yaml")).unwrap();
    assert!(yaml.contains("runtime: static"));
    assert!(yaml.contains("staticPublishPath: apps/web/dist/public"));
}
