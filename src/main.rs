use fxhash::FxBuildHasher;
use memmap2::{Advice, Mmap};
use std::{collections::{BTreeMap, HashMap}, fs::File};

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
    // Open
    let file = File::open(MEASUREMENTS_FILE_NAME).unwrap();

    let mmap = unsafe { Mmap::map(&file).unwrap() };
    mmap.advise(Advice::Sequential).unwrap();
    let mut data = &*mmap;

    // Read
    let mut stats: HashMap<Vec<u8>, Record, FxBuildHasher> = HashMap::with_capacity_and_hasher(1_000, FxBuildHasher::new());
    loop {
        if data.is_empty() {
            break
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

/// Parse line into (city, temperature and rest data)
#[inline]
fn parse_line(data: &[u8]) -> (&[u8], i16, &[u8]) {
    let (city, data) = split_on(b';', data).unwrap();
    let (temperature, data) = split_on(b'\n', data).unwrap();
    (city, parse_temperature(temperature), data)
}

#[inline]
fn split_on(chr: u8, data: &[u8]) -> Option<(&[u8], &[u8])> {
    match data.iter().position(|&c| c == chr) {
        None => None,
        Some(n) => unsafe {
            // SAFETY: returned index is guaranteed to be less than haystack.len().
            let (prefix, rest) = data.split_at_unchecked(n);
            Some((prefix, &rest.get_unchecked(1..)))
        }
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
    let (s, a, b, c ) = match buf {
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
}
