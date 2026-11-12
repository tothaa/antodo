use antodo::core::preferences::{MAX_FONT_SIZE, MIN_FONT_SIZE, Preferences};
use std::path::PathBuf;

#[test]
fn new_profile_uses_defaults_and_round_trips_every_preference() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested/preferences.json");
    assert_eq!(
        Preferences::load_from(&path).unwrap(),
        Preferences::default()
    );
    let prefs = Preferences {
        last_file: Some(PathBuf::from("/home/user/My tasks.antodo")),
        dark_mode: false,
        font_size: 22,
    };
    prefs.save_to(&path).unwrap();
    assert_eq!(Preferences::load_from(&path).unwrap(), prefs);
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1
    );
}

#[test]
fn older_profiles_gain_defaults_and_out_of_range_font_is_clamped() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    std::fs::write(&path, r#"{"dark_mode":false}"#).unwrap();
    let prefs = Preferences::load_from(&path).unwrap();
    assert!(!prefs.dark_mode);
    assert_eq!(prefs.font_size, 16);
    std::fs::write(&path, r#"{"font_size":999}"#).unwrap();
    assert_eq!(
        Preferences::load_from(&path).unwrap().font_size,
        MAX_FONT_SIZE
    );
    std::fs::write(&path, r#"{"font_size":1}"#).unwrap();
    assert_eq!(
        Preferences::load_from(&path).unwrap().font_size,
        MIN_FONT_SIZE
    );
}

#[test]
fn corrupt_profiles_and_failed_saves_preserve_existing_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    std::fs::write(&path, "not JSON").unwrap();
    assert!(Preferences::load_from(&path).is_err());
    let invalid = Preferences {
        font_size: 1,
        ..Preferences::default()
    };
    assert!(invalid.save_to(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "not JSON");
    assert!(
        Preferences::default()
            .save_to(&directory.path().join("prefs.json/child.json"))
            .is_err()
    );
}

#[test]
fn inaccessible_profile_paths_report_io_errors_and_failed_replacement_keeps_contents() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    std::fs::create_dir(&path).unwrap();
    let existing = path.join("keep.txt");
    std::fs::write(&existing, "Keep these bytes").unwrap();
    assert!(Preferences::load_from(&path).is_err());
    assert!(Preferences::default().save_to(&path).is_err());
    assert_eq!(
        std::fs::read_to_string(&existing).unwrap(),
        "Keep these bytes"
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
