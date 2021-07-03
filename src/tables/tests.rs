#[cfg(feature = "online")]
#[test]
fn test_online() {
    let table = EmojiTable::load_online((13, 0)).unwrap();

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
    lazy_static! {
            static ref HEX_SEQUENCE: Regex = Regex::new(r"[a-fA-F0-9]{1,8}").unwrap();
            static ref RANGE: Regex = Regex::new(&format!(r"(?P<range>(?P<range_start>{hex})\.\.(?P<range_end>{hex}))", hex = &*HEX_SEQUENCE)).unwrap();
            static ref SEQUENCE: Regex = Regex::new(&format!(r"(?P<sequence>({hex})(\s+({hex}))*)", hex = &*HEX_SEQUENCE)).unwrap();
            static ref EMOJI_REGEX: Regex = Regex::new(&format!(r"(?P<codepoints>{}|{})", &*RANGE, &*SEQUENCE)).unwrap();
            static ref EMOJI_KIND_REGEX: Regex = Regex::new(&format!(r"(?P<kind>{})", EmojiKind::regex())).unwrap();
            static ref DATA_REGEX: Regex = Regex::new(&format!(r"^{}\s*;\s*{}\s*(;(?P<name>.*)\s*)?(#.*)?$", &*EMOJI_REGEX, &*EMOJI_KIND_REGEX)).unwrap();
    }

    let regexr_incompatible = Regex::new(r"(\?P<[^>]+>)|(\(\?i\))").unwrap();
    let regex = format!("{}", &*DATA_REGEX);
    let regex = regexr_incompatible.replace_all(&regex, "");

    println!("{}", regex);
}