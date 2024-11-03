pub struct Utf8Encoding;

impl Utf8Encoding {
    /// 占terminal的宽度
    pub fn width(content: &str) -> usize {
        let mut result = 0;
        for ch in content.chars() {
            result += match ch.len_utf8() {
                1 | 2 => 1,
                _ => 2,
            };
        }
        result
    }
}
