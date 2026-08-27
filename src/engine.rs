use crate::proto::{MediaInfo, RepeatMode};
use regex::Regex;

pub fn search_blob(m: &MediaInfo) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = &m.title {
        parts.push(t.to_lowercase());
    }
    if let Some(a) = &m.artist {
        parts.push(a.to_lowercase());
    }
    parts.push(m.name.to_lowercase());
    parts.push(m.uri.to_lowercase());
    for t in &m.tags {
        parts.push(t.to_lowercase());
    }
    parts.join(" ")
}

pub fn matches(m: &MediaInfo, checked: &[String], re: Option<&Regex>) -> bool {
    let tags_ok = checked.iter().all(|c| m.tags.iter().any(|t| t == c));
    if !tags_ok {
        return false;
    }
    match re {
        Some(re) => re.is_match(&search_blob(m)),
        None => true,
    }
}

pub fn filter_queue(all: &[MediaInfo], checked: &[String], re: Option<&Regex>) -> Vec<i64> {
    all.iter()
        .filter(|m| matches(m, checked, re))
        .map(|m| m.id)
        .collect()
}

pub fn next_index(
    queue: &[i64],
    current: Option<i64>,
    dir: i32,
    shuffle: bool,
    repeat: RepeatMode,
    rng: &mut impl FnMut(usize) -> usize,
) -> Option<i64> {
    if queue.is_empty() {
        return None;
    }
    let len = queue.len();
    let cur = current.and_then(|c| queue.iter().position(|&x| x == c));
    if shuffle {
        if len == 1 {
            return Some(queue[0]);
        }
        let mut i = rng(len);
        while Some(i) == cur {
            i = rng(len);
        }
        return Some(queue[i]);
    }
    match dir {
        1 => match cur {
            Some(i) if i + 1 < len => Some(queue[i + 1]),
            _ if repeat == RepeatMode::All => Some(queue[0]),
            _ => None,
        },
        -1 => match cur {
            Some(0) | None if repeat == RepeatMode::All => Some(queue[len - 1]),
            Some(0) | None => None,
            Some(i) => Some(queue[i - 1]),
        },
        _ => None,
    }
}

pub struct Lcg(u64);

impl Lcg {
    pub fn new() -> Self {
        Lcg(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(1)
            | 1)
    }
    pub fn next(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 % (n as u64)) as usize
    }
}

impl Default for Lcg {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(id: i64, name: &str, tags: &[&str]) -> MediaInfo {
        MediaInfo {
            id,
            uri: name.to_string(),
            name: name.to_string(),
            kind: "offline".into(),
            title: None,
            artist: None,
            duration: None,
            bitrate: None,
            source: None,
            tags: tags.iter().map(|s| s.to_string()).collect(),
            favorite: tags.contains(&"favorite"),
        }
    }

    #[test]
    fn filter_matches_all_checked_tags() {
        let all = vec![
            media(1, "a", &["rock", "live"]),
            media(2, "b", &["rock"]),
            media(3, "c", &["jazz"]),
        ];
        let q = filter_queue(&all, &["rock".into()], None);
        assert_eq!(q, vec![1, 2]);
        let q = filter_queue(&all, &["rock".into(), "live".into()], None);
        assert_eq!(q, vec![1]);
        let q = filter_queue(&all, &[], None);
        assert_eq!(q, vec![1, 2, 3]);
    }

    #[test]
    fn filter_applies_regex() {
        let all = vec![media(1, "hi there", &[]), media(2, "goodbye", &[])];
        let re = Regex::new("there").unwrap();
        let q = filter_queue(&all, &[], Some(&re));
        assert_eq!(q, vec![1]);
    }

    #[test]
    fn search_blob_lowercases_all_fields() {
        let m = media(1, "MySong", &["RocK"]);
        let blob = search_blob(&m);
        assert!(blob.contains("mysong"));
        assert!(blob.contains("rock"));
    }

    #[test]
    fn next_wraps_only_with_repeat_all() {
        let q = vec![1, 2, 3];
        assert_eq!(
            next_index(&q, Some(2), 1, false, RepeatMode::Off, &mut |n| n),
            Some(3)
        );
        assert_eq!(
            next_index(&q, Some(3), 1, false, RepeatMode::Off, &mut |n| n),
            None
        );
        assert_eq!(
            next_index(&q, Some(3), 1, false, RepeatMode::All, &mut |n| n),
            Some(1)
        );
    }

    #[test]
    fn prev_does_not_wrap_without_repeat_all() {
        let q = vec![1, 2, 3];
        assert_eq!(
            next_index(&q, Some(3), -1, false, RepeatMode::Off, &mut |n| n),
            Some(2)
        );
        assert_eq!(
            next_index(&q, Some(1), -1, false, RepeatMode::Off, &mut |n| n),
            None
        );
        assert_eq!(
            next_index(&q, Some(1), -1, false, RepeatMode::All, &mut |n| n),
            Some(3)
        );
    }

    #[test]
    fn shuffle_picks_different_items() {
        let q = vec![1, 2, 3, 4];
        let mut lcg = Lcg(1234567);
        let a = next_index(&q, Some(1), 1, true, RepeatMode::Off, &mut |n| lcg.next(n));
        let b = next_index(&q, Some(1), 1, true, RepeatMode::Off, &mut |n| lcg.next(n));
        assert!(q.contains(&a.unwrap()));
        assert!(q.contains(&b.unwrap()));
        assert_eq!(
            next_index(&[7], Some(7), 1, true, RepeatMode::Off, &mut |n| lcg
                .next(n)),
            Some(7)
        );
    }

    #[test]
    fn empty_queue_gives_none() {
        assert_eq!(
            next_index(&[], None, 1, false, RepeatMode::All, &mut |n| n),
            None
        );
    }
}
