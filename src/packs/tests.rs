use crate::packs::pack::{EmojiPack};
use std::path::PathBuf;
use crate::emojis::emoji::Emoji;
use crate::loadables::loadable::Loadable;
use crate::loadables::sources::fs_source::FsSource;
use crate::tests::init_logger;

const TEST_PACK: &str = "test_files/packs/basic_pack/pack.json";

#[test]
fn test_load_pack() {
    init_logger();

    let path = PathBuf::from(TEST_PACK);
    let source = FsSource::new(path).unwrap();
    let pack = EmojiPack::load(source).unwrap();

    assert_eq!(pack.name.as_ref().unwrap(), "Test-Pack");
    assert_eq!(pack.table.len(), 1);

    let screaming_face = Emoji::from_u32_sequence(vec![0x1f631], None).unwrap();

    assert_eq!(pack.validate().0.unwrap_err(), vec![screaming_face]);
}