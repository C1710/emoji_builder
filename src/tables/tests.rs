
use crate::emojis::emoji_kind::EmojiKind;
use regex::Regex;
use crate::tables::regexes::{data_regex, test_regex};
use crate::tables::online::load_online_table;

#[cfg(feature = "online")]
#[test]
fn test_online() {
    let table = load_online_table((13, 0)).unwrap();

    let kissing_face = vec![0x1f617];
    let smiling_face = vec![0x263a, 0xfe0f];
    let woman_medium_skin_tone_white_hair = vec![0x1f469, 0x1f3fd, 0x200d, 0x1f9b3];

    assert_eq!(table.get_codepoint_by_name("kissing face"), kissing_face);
    assert_eq!(table.get_codepoint_by_name("Smiling Face"), smiling_face);
    assert_eq!(table.get_codepoint_by_name("woman: medium skin tone, white hair"), woman_medium_skin_tone_white_hair);
    assert_eq!(table.get_codepoint_by_name("woman medium SkiN ToNe WhITe hair"), woman_medium_skin_tone_white_hair);

    assert_eq!(
        table.get_by_name("woman: medium skin tone, white hair").unwrap().1.0,
        vec![EmojiKind::EmojiZwjSequence]
    );

    assert!(table.get_by_name("woman").is_some());

    assert_eq!(
        table.get_by_name("woman").unwrap().1.0,
        vec![
            EmojiKind::Emoji,
            EmojiKind::ModifierBase,
            EmojiKind::EmojiPresentation,
            EmojiKind::ExtendedPictographic
        ]
    );
}


#[test]
fn print_regexes() {
    let data_regex = data_regex();
    let test_regex = test_regex();

    let regexr_incompatible = Regex::new(r"(\?P<[^>]+>)|(\(\?i\))").unwrap();
    let data_regex = format!("{}", data_regex);
    let test_regex = format!("{}", test_regex);
    let data_regex = regexr_incompatible.replace_all(&data_regex, "");
    let test_regex = regexr_incompatible.replace_all(&test_regex, "");

    println!("{}\n{}", data_regex, test_regex);
}