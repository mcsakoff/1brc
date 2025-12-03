use fxhash::FxBuildHasher;
use memchr::memchr;
use memmap2::{Advice, Mmap};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    num::NonZero,
};

struct Record {
    min: i16,
    max: i16,
    sum: i32,
    count: usize,
}

impl Record {
    fn new(value: i16) -> Self {
        Self {
            min: value,
            max: value,
            sum: value as i32,
            count: 1,
        }
    }

    fn add(&mut self, value: i16) {
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value as i32;
        self.count += 1;
    }

    fn merge(&mut self, other: &Self) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.sum += other.sum;
        self.count += other.count;
    }

    fn min(&self) -> f32 {
        self.min as f32 / 10.0
    }

    fn max(&self) -> f32 {
        self.max as f32 / 10.0
    }

    fn avg(&self) -> f32 {
        self.sum as f32 / self.count as f32 / 10.0
    }
}

const MEASUREMENTS_FILE_NAME: &str = "measurements.txt";

fn main() {
    // Open data file and map to memory
    let file = File::open(MEASUREMENTS_FILE_NAME).unwrap();
    let mmap = unsafe { Mmap::map(&file).unwrap() };
    mmap.advise(Advice::Sequential).unwrap();
    let data = &*mmap;

    // Process data
    let stats = std::thread::scope(move |s| {
        let mut all_stats: HashMap<Vec<u8>, Record, FxBuildHasher> = HashMap::with_capacity_and_hasher(1_000, FxBuildHasher::new());

        let num_threads = std::thread::available_parallelism().unwrap();
        let (tx, rx) = std::sync::mpsc::sync_channel(num_threads.get());
        for chunk in split_to_aligned_chunks(data, num_threads) {
            let tx = tx.clone();
            s.spawn(move || {
                let stats = process_chunk(chunk);
                tx.send(stats).unwrap();
            });
        }
        drop(tx);

        // Collect partial stats from all threads into final stats merging them together.
        for stats in rx {
            for (city, rec) in stats {
                all_stats
                    .entry(city)
                    .and_modify(|r| r.merge(&rec))
                    .or_insert(rec);
            }
        }
        all_stats
    });

    // Collect and sort
    let stats: BTreeMap<Vec<u8>, Record> = stats.into_iter().collect();

    // Output results
    print!("{{");
    let mut stats = stats.into_iter().peekable();
    while let Some((city, r)) = stats.next() {
        let city = unsafe { std::str::from_utf8_unchecked(&city) };
        print!("{}={:.1}/{:.1}/{:.1}", city, r.min(), r.avg(), r.max());
        if stats.peek().is_some() {
            print!(", ");
        }
    }
    println!("}}");
}

/// Split initial data to chunks aligned to '\n' character.
/// It returns at most `num` chunks.
fn split_to_aligned_chunks(data: &[u8], num: NonZero<usize>) -> Vec<&[u8]> {
    let num = num.get();
    if num == 1 {
        return vec![data];
    }

    let estimated_chunk_size = data.len() / num;

    let mut data = data;
    let mut chunks = Vec::with_capacity(num);
    for _ in 0..num {
        if data.is_empty() {
            break;
        }
        match split_after_offset(b'\n', data, estimated_chunk_size) {
            Some((chunk, rest)) => {
                chunks.push(chunk);
                data = rest;
            }
            None => {
                // Not found where to split
                chunks.push(data);
                break;
            }
        }
    }
    chunks
}

/// Process data chunk
fn process_chunk(data: &[u8]) -> HashMap<Vec<u8>, Record, FxBuildHasher> {
    let mut data = data;
    let mut stats: HashMap<Vec<u8>, Record, FxBuildHasher> = HashMap::with_capacity_and_hasher(1_000, FxBuildHasher::new());
    loop {
        if data.is_empty() {
            break;
        }
        let (city, temperature, rest) = parse_line(data);
        // println!("{}: {}", std::str::from_utf8(&city).unwrap(), temperature as f32 / 10.0);
        match stats.get_mut(city) {
            Some(r) => {
                r.add(temperature);
            }
            None => {
                stats.insert(city.to_vec(), Record::new(temperature));
            }
        }
        data = rest;
    }
    stats
}

/// Parse line into (city, temperature and rest data)
#[inline]
fn parse_line(data: &[u8]) -> (&[u8], i16, &[u8]) {
    let (city, data) = split_on(b';', data).unwrap();
    let (temperature, data) = split_on(b'\n', data).unwrap();
    (city, parse_temperature(temperature), data)
}

/// Split at the first occurrence of `chr`. Removes the character from the result.
#[inline]
fn split_on(chr: u8, data: &[u8]) -> Option<(&[u8], &[u8])> {
    match memchr(chr, data) {
        None => None,
        Some(n) => unsafe {
            // SAFETY: returned index is guaranteed to be less than haystack.len().
            let (prefix, rest) = data.split_at_unchecked(n);
            Some((prefix, &rest.get_unchecked(1..)))
        },
    }
}

/// Split at the first occurrence of `chr` after `offset`. Lefts the `chr` character in first part.
#[inline]
fn split_after_offset(chr: u8, data: &[u8], offset: usize) -> Option<(&[u8], &[u8])> {
    if offset >= data.len() {
        return Some((data, &[]));
    }
    match memchr(chr, &data[offset..]) {
        None => None,
        Some(n) => unsafe {
            // SAFETY: returned index is guaranteed to be less than haystack.len().
            Some(data.split_at_unchecked(offset + n + 1))
        },
    }
}

fn parse_temperature(buf: &[u8]) -> i16 {
    assert!(buf.len() >= 3);
    assert!(buf.len() <= 5);

    #[inline]
    fn chr2num(b: &u8) -> i16 {
        (*b - b'0') as i16
    }

    // Parse number -99.9 as
    //              sab.c
    let (s, a, b, c) = match buf {
        // 2.3
        [b, b'.', c] => (1, 0, chr2num(b), chr2num(c)),
        // -2.3
        [b'-', b, b'.', c] => (-1, 0, chr2num(b), chr2num(c)),
        // 12.3
        [a, b, b'.', c] => (1, chr2num(a), chr2num(b), chr2num(c)),
        // -12.3
        [b'-', a, b, b'.', c] => (-1, chr2num(a), chr2num(b), chr2num(c)),
        _ => panic!("Invalid temperature format: '{}' ({:?})", String::from_utf8_lossy(buf), buf),
    };
    s * ((100 * a + 10 * b) + c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record() {
        let mut record = Record::new(10);
        record.add(20);
        assert_eq!(record.min, 10);
        assert_eq!(record.max, 20);
        assert_eq!(record.sum, 30);
        assert_eq!(record.count, 2);
        assert_eq!(record.min(), 1.0);
        assert_eq!(record.max(), 2.0);
        assert_eq!(record.avg(), 1.5);
    }

    #[test]
    fn test_records_merge() {
        let mut record1 = Record::new(10);
        record1.add(20);
        let record2 = Record::new(30);
        record1.merge(&record2);
        assert_eq!(record1.min, 10);
        assert_eq!(record1.max, 30);
        assert_eq!(record1.sum, 60);
        assert_eq!(record1.count, 3);
    }

    #[test]
    fn test_parse_temperature() {
        assert_eq!(parse_temperature(b"0.0"), 0);

        assert_eq!(parse_temperature(b"0.1"), 1);
        assert_eq!(parse_temperature(b"1.0"), 10);
        assert_eq!(parse_temperature(b"10.0"), 100);
        assert_eq!(parse_temperature(b"99.9"), 999);

        assert_eq!(parse_temperature(b"-0.1"), -1);
        assert_eq!(parse_temperature(b"-1.0"), -10);
        assert_eq!(parse_temperature(b"-10.0"), -100);
        assert_eq!(parse_temperature(b"-99.9"), -999);
    }

    #[test]
    fn test_split_on() {
        assert_eq!(
            split_on(b';', b"123;456;".as_slice()),
            Some((b"123".as_slice(), b"456;".as_slice()))
        );
    }

    #[test]
    fn test_split_after() {
        assert_eq!(
            split_after_offset(b'\n', b"123\n456\n789\n".as_slice(), 0),
            Some((b"123\n".as_slice(), b"456\n789\n".as_slice()))
        );
        assert_eq!(
            split_after_offset(b'\n', b"123\n456\n789\n".as_slice(), 5),
            Some((b"123\n456\n".as_slice(), b"789\n".as_slice()))
        );
        assert_eq!(
            split_after_offset(b'\n', b"123\n456\n789\n".as_slice(), 10),
            Some((b"123\n456\n789\n".as_slice(), b"".as_slice()))
        );
        assert_eq!(
            split_after_offset(b'\n', b"123\n456\n789\n".as_slice(), 20),
            Some((b"123\n456\n789\n".as_slice(), b"".as_slice()))
        );
    }

    #[test]
    fn test_split_to_aligned_chunks() {
        let data = b"123\n456\n789\n";
        let chunks = split_to_aligned_chunks(data, NonZero::new(2).unwrap());
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], b"123\n456\n");
        assert_eq!(chunks[1], b"789\n");
    }
}
