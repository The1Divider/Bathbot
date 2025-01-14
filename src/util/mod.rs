use std::{io::Cursor, mem};

use eyre::Result;
use futures::stream::{FuturesOrdered, StreamExt};
use image::{
    imageops::FilterType, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat::Png,
};
use tokio::time::Duration;

use crate::Context;

pub use self::{
    bitflags::*, boyer_moore::*, country_code::CountryCode, cow::CowUtils, emote::Emote, ext::*,
    html_to_png::*, matrix::Matrix, monthly::Monthly,
};

pub mod builder;
pub mod constants;
pub mod datetime;
pub mod hasher;
pub mod interaction;
pub mod matcher;
pub mod matrix;
pub mod numbers;
pub mod osu;
pub mod query;

mod bitflags;
mod boyer_moore;
mod country_code;
mod cow;
mod emote;
mod ext;
mod html_to_png;
mod monthly;

macro_rules! get {
    ($slice:ident[$idx:expr]) => {
        unsafe { *$slice.get_unchecked($idx) }
    };
}

macro_rules! set {
    ($slice:ident[$idx:expr] = $val:expr) => {
        unsafe { *$slice.get_unchecked_mut($idx) = $val }
    };
}

pub fn levenshtein_similarity(word_a: &str, word_b: &str) -> f32 {
    let (dist, len) = levenshtein_distance(word_a, word_b);

    (len - dist) as f32 / len as f32
}

/// "How many replace/delete/insert operations are necessary to morph one word into the other?"
///
/// Returns (distance, max word length) tuple
pub fn levenshtein_distance<'w>(mut word_a: &'w str, mut word_b: &'w str) -> (usize, usize) {
    let m = word_a.chars().count();
    let mut n = word_b.chars().count();

    if m > n {
        mem::swap(&mut word_a, &mut word_b);
        n = m;
    }

    // u16 is sufficient considering the max length
    // of discord messages is smaller than u16::MAX
    let mut costs: Vec<_> = (0..=n as u16).collect();

    // SAFETY for get! and set!:
    // chars(word_a) <= chars(word_b) = n < n + 1 = costs.len()

    for (a, i) in word_a.chars().zip(1..) {
        let mut last_val = i;

        for (b, j) in word_b.chars().zip(1..) {
            let new_val = if a == b {
                get!(costs[j - 1])
            } else {
                get!(costs[j - 1]).min(last_val).min(get!(costs[j])) + 1
            };

            set!(costs[j - 1] = last_val);
            last_val = new_val;
        }

        set!(costs[n] = last_val);
    }

    (get!(costs[n]) as usize, n)
}

/// Consider the length of the longest common substring, then repeat recursively
/// for the remaining left and right parts of the words
pub fn gestalt_pattern_matching(word_a: &str, word_b: &str) -> f32 {
    let chars_a = word_a.chars().count();
    let chars_b = word_b.chars().count();

    // u16 is sufficient considering the max length
    // of discord messages is smaller than u16::MAX
    let mut buf = vec![0; chars_a.max(chars_b) + 1];

    // SAFETY: buf.len is set to be 1 + max(chars(word_a), chars(word_b))
    let matching_chars = unsafe { _gestalt_pattern_matching(word_a, word_b, &mut buf) };

    (2 * matching_chars) as f32 / (chars_a + chars_b) as f32
}

/// Caller must guarantee that buf.len is 1 + max(chars(word_a), chars(word_b))
unsafe fn _gestalt_pattern_matching(word_a: &str, word_b: &str, buf: &mut [u16]) -> usize {
    let SubstringResult {
        start_a,
        start_b,
        len,
    } = longest_common_substring(word_a, word_b, buf);

    if len == 0 {
        return 0;
    }

    let mut matches = len;

    if start_a > 0 && start_b > 0 {
        let prefix_a = prefix(word_a, start_a);
        let prefix_b = prefix(word_b, start_b);
        matches += _gestalt_pattern_matching(prefix_a, prefix_b, buf);
    }

    let suffix_a = suffix(word_a, start_a + len);
    let suffix_b = suffix(word_b, start_b + len);

    if !(suffix_a.is_empty() || suffix_b.is_empty()) {
        matches += _gestalt_pattern_matching(suffix_a, suffix_b, buf);
    }

    matches
}

fn prefix(s: &str, len: usize) -> &str {
    let mut indices = s.char_indices();
    let end = indices.nth(len).map_or_else(|| s.len(), |(i, _)| i);

    // SAFETY: `end` is provided by `char_indices` which ensues valid char bounds
    unsafe { s.get_unchecked(..end) }
}

fn suffix(s: &str, start: usize) -> &str {
    let mut indices = s.char_indices();
    let start = indices.nth(start).map_or_else(|| s.len(), |(i, _)| i);

    // SAFETY: `start` is provided by `char_indices` which ensues valid char bounds
    unsafe { s.get_unchecked(start..) }
}

/// Caller must guarantee that buf.len >= 1 + max(chars(word_a), chars(word_b))
unsafe fn longest_common_substring<'w>(
    mut word_a: &'w str,
    mut word_b: &'w str,
    buf: &mut [u16],
) -> SubstringResult {
    if word_a.is_empty() || word_b.is_empty() {
        return SubstringResult::default();
    }

    let mut swapped = false;
    let mut m = word_a.chars().count();
    let mut n = word_b.chars().count();

    // Ensure word_b being the longer word with length n
    if m > n {
        mem::swap(&mut word_a, &mut word_b);
        mem::swap(&mut m, &mut n);
        swapped = true;
    }

    let mut len = 0;
    let mut start_b = 0;
    let mut end_a = 0;

    // SAFETY for indices:
    // i ranges from 0 to n - 1 so the indices range from 0 to n
    // No issue since buf.len = n + 1, as guaranteed by the caller

    for (j, a) in word_a.chars().rev().enumerate() {
        for (i, b) in word_b.chars().enumerate() {
            if a != b {
                *buf.get_unchecked_mut(i) = 0;

                continue;
            }

            let val = *buf.get_unchecked(i + 1) + 1;
            *buf.get_unchecked_mut(i) = val;

            if val > len {
                len = val;
                start_b = i;
                end_a = j;
            }
        }
    }

    let (start_a, start_b) = if swapped {
        (start_b, m - end_a - 1)
    } else {
        (m - end_a - 1, start_b)
    };

    // Reset the buffer
    for elem in buf.iter_mut().take(n) {
        *elem = 0;
    }

    SubstringResult {
        start_a,
        start_b,
        len: len as usize,
    }
}

#[derive(Default)]
struct SubstringResult {
    start_a: usize,
    start_b: usize,
    len: usize,
}

pub async fn get_combined_thumbnail<'s>(
    ctx: &Context,
    avatar_urls: impl IntoIterator<Item = &'s str>,
    amount: u32,
    width: Option<u32>,
) -> Result<Vec<u8>> {
    let width = width.map_or(128, |w| w.max(128));
    let mut combined = DynamicImage::new_rgba8(width, 128);
    let w = (width / amount).min(128);
    let total_offset = (width - amount * w) / 2;

    // Future stream
    let mut pfp_futs: FuturesOrdered<_> = avatar_urls
        .into_iter()
        .map(|url| ctx.client().get_avatar(url))
        .collect();

    let mut next = pfp_futs.next().await;
    let mut i = 0;

    // Closure that stitches the stripe onto the combined image
    let mut img_combining = |img: DynamicImage, i: u32| {
        let img = img.resize_exact(128, 128, FilterType::Lanczos3);

        let dst_offset = total_offset + i * w;
        let src_offset = (w < 128) as u32 * i * (128 - w) / amount;

        for i in 0..w {
            for j in 0..128 {
                let pixel = img.get_pixel(src_offset + i, j);
                combined.put_pixel(dst_offset + i, j, pixel);
            }
        }
    };

    // Process the stream elements
    while let Some(pfp_result) = next {
        let pfp = pfp_result?;
        let img = image::load_from_memory(&pfp)?;
        let (res, _) = tokio::join!(pfp_futs.next(), async { img_combining(img, i) });
        next = res;
        i += 1;
    }

    let capacity = width as usize * 128;
    let png_bytes: Vec<u8> = Vec::with_capacity(capacity);
    let mut cursor = Cursor::new(png_bytes);
    combined.write_to(&mut cursor, Png)?;

    Ok(cursor.into_inner())
}

#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    current: Duration,
    base: u32,
    factor: u32,
    max_delay: Option<Duration>,
}

impl ExponentialBackoff {
    pub fn new(base: u32) -> Self {
        ExponentialBackoff {
            current: Duration::from_millis(base as u64),
            base,
            factor: 1,
            max_delay: None,
        }
    }

    pub fn factor(mut self, factor: u32) -> Self {
        self.factor = factor;

        self
    }

    pub fn max_delay(mut self, max_delay: u64) -> Self {
        self.max_delay.replace(Duration::from_millis(max_delay));

        self
    }
}

impl Iterator for ExponentialBackoff {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        let duration = self.current * self.factor;

        if let Some(max_delay) = self.max_delay.filter(|&max_delay| duration > max_delay) {
            return Some(max_delay);
        }

        self.current *= self.base;

        Some(duration)
    }
}
