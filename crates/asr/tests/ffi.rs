use std::path::PathBuf;
use ustt_asr::ParakeetLib;

fn lib_path() -> PathBuf {
    let raw = std::env::var("PARAKEET_LIB").unwrap_or_else(|_| {
        if cfg!(windows) {
            "lib/parakeet.dll".into()
        } else {
            "lib/libparakeet.so".into()
        }
    });
    if PathBuf::from(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(raw)
    }
}

#[test]
#[ignore = "needs libparakeet"]
fn abi_is_ten() {
    let lib = ParakeetLib::open(&lib_path()).unwrap();
    assert!(lib.abi_version() >= 10);
}
