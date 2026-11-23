//! 두벌식 한글 조합 — 가상 키보드가 자모를 받아 글을 만든다. 만든 글은 `Converter::text` 비교로 호스트에 간다.
//! 호스트 IME 상태와 상관없이 같은 글이 들어가게 웹앱이 직접 조합한다(research/touch-control.md 결론 4).
//! 브라우저를 모르는 순수 코드 — native `cargo test -p web`.

const CHO: &[char] = &[
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];
const JUNG: &[char] = &[
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];
/// 0 = 받침 없음.
const JONG: &[char] = &[
    '\0', 'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ',
    'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];
/// 겹모음·겹받침 = (앞, 뒤, 합친 것).
const VOWEL_PAIRS: &[(char, char, char)] = &[
    ('ㅗ', 'ㅏ', 'ㅘ'),
    ('ㅗ', 'ㅐ', 'ㅙ'),
    ('ㅗ', 'ㅣ', 'ㅚ'),
    ('ㅜ', 'ㅓ', 'ㅝ'),
    ('ㅜ', 'ㅔ', 'ㅞ'),
    ('ㅜ', 'ㅣ', 'ㅟ'),
    ('ㅡ', 'ㅣ', 'ㅢ'),
];
const FINAL_PAIRS: &[(char, char, char)] = &[
    ('ㄱ', 'ㅅ', 'ㄳ'),
    ('ㄴ', 'ㅈ', 'ㄵ'),
    ('ㄴ', 'ㅎ', 'ㄶ'),
    ('ㄹ', 'ㄱ', 'ㄺ'),
    ('ㄹ', 'ㅁ', 'ㄻ'),
    ('ㄹ', 'ㅂ', 'ㄼ'),
    ('ㄹ', 'ㅅ', 'ㄽ'),
    ('ㄹ', 'ㅌ', 'ㄾ'),
    ('ㄹ', 'ㅍ', 'ㄿ'),
    ('ㄹ', 'ㅎ', 'ㅀ'),
    ('ㅂ', 'ㅅ', 'ㅄ'),
];

fn idx(t: &[char], c: char) -> Option<usize> {
    t.iter().position(|&x| x == c)
}
fn join(pairs: &[(char, char, char)], a: char, b: char) -> Option<char> {
    pairs.iter().find(|p| p.0 == a && p.1 == b).map(|p| p.2)
}
fn split(pairs: &[(char, char, char)], c: char) -> Option<(char, char)> {
    pairs.iter().find(|p| p.2 == c).map(|p| (p.0, p.1))
}
fn syllable(cho: usize, jung: usize, jong: usize) -> char {
    char::from_u32(0xAC00 + ((cho * 21 + jung) * 28 + jong) as u32).unwrap()
}
/// 완성 음절 → (초성, 중성, 종성) 번호.
fn parts(c: char) -> Option<(usize, usize, usize)> {
    let n = (c as u32).checked_sub(0xAC00).filter(|&n| n < 11172)? as usize;
    Some((n / 588, n % 588 / 28, n % 28))
}

pub fn is_jamo(c: char) -> bool {
    ('ㄱ'..='ㅣ').contains(&c)
}

/// 조합 중인 글. `buf` = 마지막으로 지운 뒤 친 글 전부(호스트에 보낼 글과 같다).
#[derive(Default)]
pub struct Hangul {
    pub buf: String,
    /// 마지막 글자가 아직 조합 중인가 — 아니면 새 자모가 앞 글자에 붙지 않는다.
    composing: bool,
}

impl Hangul {
    pub fn clear(&mut self) {
        self.buf.clear();
        self.composing = false;
    }

    fn replace_last(&mut self, c: char) {
        self.buf.pop();
        self.buf.push(c);
    }

    /// 조합을 끝낸다(한/영 바꿈) — 다음 자모는 새 글자로 시작한다.
    pub fn commit(&mut self) {
        self.composing = false;
    }

    /// 한글이 아닌 글자(영문·숫자·공백) — 조합을 끝내고 붙인다.
    pub fn push_char(&mut self, c: char) {
        self.composing = false;
        self.buf.push(c);
    }

    /// 자모 하나(두벌식 자판 한 번).
    pub fn push_jamo(&mut self, j: char) {
        let last = self.buf.chars().last().filter(|_| self.composing);
        self.composing = true;
        let vowel = idx(JUNG, j).is_some();
        match (last, vowel) {
            (Some(l), false) => {
                if let Some((cho, jung, jong)) = parts(l) {
                    if jong == 0 {
                        if let Some(t) = idx(JONG, j) {
                            return self.replace_last(syllable(cho, jung, t));
                        }
                    } else if let Some(t) =
                        join(FINAL_PAIRS, JONG[jong], j).and_then(|c| idx(JONG, c))
                    {
                        return self.replace_last(syllable(cho, jung, t));
                    }
                }
            }
            (Some(l), true) => {
                if let Some((cho, jung, jong)) = parts(l) {
                    let v = idx(JUNG, j).unwrap();
                    if jong != 0 {
                        // 받침이 다음 음절의 첫소리로 넘어간다: 안 + ㅏ = 아나, 앉 + ㅏ = 안자.
                        let (keep, moved) = match split(FINAL_PAIRS, JONG[jong]) {
                            Some((a, b)) => (idx(JONG, a).unwrap(), b),
                            None => (0, JONG[jong]),
                        };
                        self.replace_last(syllable(cho, jung, keep));
                        self.buf.push(syllable(idx(CHO, moved).unwrap(), v, 0));
                        return;
                    }
                    if let Some(u) = join(VOWEL_PAIRS, JUNG[jung], j).and_then(|c| idx(JUNG, c)) {
                        return self.replace_last(syllable(cho, u, 0));
                    }
                } else if let Some(cho) = idx(CHO, l) {
                    return self.replace_last(syllable(cho, idx(JUNG, j).unwrap(), 0));
                } else if let Some(u) = join(VOWEL_PAIRS, l, j) {
                    return self.replace_last(u);
                }
            }
            (None, _) => {}
        }
        self.buf.push(j);
    }

    /// 지우기. 조합 중이면 자모 하나(안 → 아), 아니면 글자 하나. 지울 글이 없으면 false(호스트에 Backspace 키를 보낸다).
    pub fn backspace(&mut self) -> bool {
        let Some(l) = self.buf.chars().last() else {
            self.composing = false;
            return false;
        };
        if !self.composing {
            self.buf.pop();
            return true;
        }
        if let Some((cho, jung, jong)) = parts(l) {
            let c = if jong != 0 {
                let keep = split(FINAL_PAIRS, JONG[jong]).map_or(0, |(a, _)| idx(JONG, a).unwrap());
                syllable(cho, jung, keep)
            } else if let Some((a, _)) = split(VOWEL_PAIRS, JUNG[jung]) {
                syllable(cho, idx(JUNG, a).unwrap(), 0)
            } else {
                CHO[cho]
            };
            self.replace_last(c);
        } else if let Some((a, _)) = split(VOWEL_PAIRS, l) {
            self.replace_last(a);
        } else {
            self.buf.pop();
            self.composing = false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 두벌식 자판 글자(영문 자리) → 자모.
    fn typed(keys: &str) -> String {
        const EN: &str = "qwertyuiopasdfghjklzxcvbnmQWERTOP";
        const KO: &str = "ㅂㅈㄷㄱㅅㅛㅕㅑㅐㅔㅁㄴㅇㄹㅎㅗㅓㅏㅣㅋㅌㅊㅍㅠㅜㅡㅃㅉㄸㄲㅆㅒㅖ";
        let mut h = Hangul::default();
        for k in keys.chars() {
            match EN.chars().position(|e| e == k) {
                Some(i) => h.push_jamo(KO.chars().nth(i).unwrap()),
                None => h.push_char(k),
            }
        }
        h.buf
    }

    #[test]
    fn sc006_sentence_from_dubeolsik_keys() {
        // 정답지: 두벌식 표준 배열로 「안녕하세요 원격 테스트」를 치는 자판 순서.
        assert_eq!(
            typed("dkssudgktpdy dnjsrur xptmxm"),
            "안녕하세요 원격 테스트"
        );
    }

    #[test]
    fn final_moves_to_next_syllable_and_double_finals() {
        assert_eq!(typed("dkswk"), "안자");
        assert_eq!(typed("dkswdk"), "앉아");
        assert_eq!(typed("rkqtl"), "갑시"); // 값 + ㅣ → 갑 + 시
        assert_eq!(typed("rkqtdl"), "값이");
        assert_eq!(typed("dhkd"), "왕");
        assert_eq!(typed("Rk"), "까");
        assert_eq!(typed("k"), "ㅏ");
        assert_eq!(typed("hk"), "ㅘ");
    }

    #[test]
    fn backspace_removes_one_jamo_while_composing() {
        let mut h = Hangul::default();
        for j in ['ㅇ', 'ㅏ', 'ㄴ', 'ㅈ'] {
            h.push_jamo(j);
        }
        assert_eq!(h.buf, "앉");
        let steps: Vec<String> = (0..4)
            .map(|_| {
                h.backspace();
                h.buf.clone()
            })
            .collect();
        assert_eq!(steps, ["안", "아", "ㅇ", ""]);
        assert!(!h.backspace(), "빈 글이면 호스트 Backspace 로");
    }

    #[test]
    fn finished_word_is_not_rejoined() {
        let mut h = Hangul::default();
        h.push_jamo('ㄱ');
        h.push_jamo('ㅏ');
        h.push_char(' ');
        h.push_jamo('ㅏ');
        assert_eq!(h.buf, "가 ㅏ");
        // 조합이 끝난 글자는 통째로 지운다.
        h.backspace();
        h.backspace();
        h.backspace();
        assert_eq!(h.buf, "");
    }
}
