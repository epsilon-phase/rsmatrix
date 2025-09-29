pub fn ascii_chars() -> Vec<char> {
    let mut q: Vec<char> = Vec::new();
    for i in 0u8..255u8 {
        q.push(i as char);
    }
    q.iter()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| *c)
        .collect()
}
pub fn hiragana_chars() -> Vec<char> {
    let mut q: Vec<char> = Vec::new();
    for i in 0x3040..=0x309F {
        if i == 0x3040 || i == 0x3097 || i == 0x3098 {
            continue;
        }
        q.push(char::from_u32(i).unwrap());
    }
    q.drain(..).collect()
}
pub fn cjk_chars()->Vec<char>{
    let mut q = Vec::new();
    for i in 0x4E00..=0x9FFF{
        q.push(char::from_u32(i).unwrap());
    }
    q
}