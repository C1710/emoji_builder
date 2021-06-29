use crate::packs::pack::EmojiPack;
use std::path::PathBuf;
use crate::loadable::Loadable;
use crate::emoji::Emoji;

const TEST_PACK: &str = "test_files/packs/basic_pack/pack.json";

#[test]
fn test_load_pack() {
    let path = PathBuf::from(TEST_PACK);
    let pack = EmojiPack::from_file(&path).unwrap();

    assert_eq!(pack.name.as_ref().unwrap(), "Test-Pack");
    assert_eq!(pack.table.len(), 1);

    let screaming_face = Emoji::from_u32_sequence(vec![0x1f631], None).unwrap();

    assert_eq!(pack.validate().0.unwrap_err(), vec![screaming_face]);
}