#![allow(clippy::unwrap_used, clippy::expect_used)] // Test assertions.
use std::{fs, path::PathBuf};
#[test]
fn bounded_catalog_rejects_empty_links_and_entry_overflow() {
    let target = PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").expect("explicit target"));
    let root = target.join(format!("catalog-test-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    assert!(knot_server::catalog::load(&root).is_err());
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden/T03/model.json"),
        root.join("model.json"),
    )
    .unwrap();
    assert_eq!(knot_server::catalog::load(&root).unwrap().len(), 1);
    for i in 0..1023 {
        fs::write(root.join(format!("ignored-{i}")), b"").unwrap();
    }
    assert_eq!(knot_server::catalog::load(&root).unwrap().len(), 1);
    fs::write(root.join("ignored-overflow"), b"").unwrap();
    assert_eq!(
        knot_server::catalog::load(&root).err().unwrap().details["limit"],
        "directoryEntries"
    );
    fs::remove_file(root.join("ignored-overflow")).unwrap();
    for i in 0..1023 {
        fs::remove_file(root.join(format!("ignored-{i}"))).unwrap();
    }
    fs::create_dir(root.join("not-a-file.json")).unwrap();
    assert!(knot_server::catalog::load(&root).is_err());
    fs::remove_dir(root.join("not-a-file.json")).unwrap();
    for i in 0..64 {
        fs::copy(root.join("model.json"), root.join(format!("copy-{i}.json"))).unwrap();
    }
    assert_eq!(
        knot_server::catalog::load(&root).err().unwrap().details["limit"],
        "models"
    );
    for i in 0..64 {
        fs::remove_file(root.join(format!("copy-{i}.json"))).unwrap();
    }
    fs::remove_file(root.join("model.json")).unwrap();
    fs::remove_dir(root).unwrap();
}
