use git_mirror::do_mirror;
use git_mirror::provider::{Mirror, MirrorError, MirrorResult, Provider};
use git_mirror::MirrorOptions;
use std::cell::RefCell;
use std::path::PathBuf;
use tempfile::TempDir;

struct MockProvider {
    label: String,
    repos: RefCell<Vec<MirrorResult>>,
}

impl Provider for MockProvider {
    fn get_mirror_repos(&self) -> std::result::Result<Vec<MirrorResult>, String> {
        let mut v = self.repos.borrow_mut();
        let mut result = Vec::new();
        std::mem::swap(&mut result, &mut *v);
        Ok(result)
    }
    fn get_label(&self) -> String {
        self.label.clone()
    }
}

fn default_opts(mirror_dir: PathBuf) -> MirrorOptions {
    MirrorOptions {
        mirror_dir,
        dry_run: false,
        metrics_file: None,
        junit_file: None,
        worker_count: 1,
        git_executable: "git".to_string(),
        refspec: None,
        remove_workrepo: false,
        fail_on_sync_error: false,
        mirror_lfs: false,
        git_timeout: None,
    }
}

#[test]
fn test_do_mirror_end_to_end() {
    let dir = TempDir::new().unwrap();
    let metrics_path = dir.path().join("metrics.prom");
    let junit_path = dir.path().join("junit.xml");

    let opts = MirrorOptions {
        metrics_file: Some(metrics_path.clone()),
        junit_file: Some(junit_path.clone()),
        ..default_opts(dir.path().to_path_buf())
    };

    let provider = MockProvider {
        label: "e2e".to_string(),
        repos: RefCell::new(vec![
            MirrorResult::Err(MirrorError::Skip("https://example.com/skip".to_string())),
            MirrorResult::Ok(Mirror {
                origin: "https://example.com/a".to_string(),
                destination: "https://example.com/b".to_string(),
                refspec: None,
                lfs: false,
            }),
        ]),
    };

    let result = do_mirror(Box::new(provider), &opts);
    assert!(result.is_ok());
    assert!(metrics_path.exists());
    assert!(junit_path.exists());
}
