use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
};
use std::collections::BTreeMap;

struct Record {
    min: f32,
    max: f32,
    sum: f32,
    count: usize,
}

impl Record {
    fn default() -> Self {
        Self {
            min: f32::MAX,
            max: f32::MIN,
            sum: 0.0,
            count: 0,
        }
    }

    fn add(&mut self, value: f32) {
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.count += 1;
    }

    fn avg(&self) -> f32 {
        self.sum / self.count as f32
    }
}

const MEASUREMENTS_FILE_NAME: &str = "measurements.txt";

fn main() {
    // Open
    let file = File::open(MEASUREMENTS_FILE_NAME).unwrap();
    let file = BufReader::new(file);

    // Read
    let mut stats: HashMap<String, Record> = HashMap::new();
    for line in file.lines() {
        let line = line.unwrap();
        let (city, temperature) = line.split_once(';').unwrap();
        let city = city.to_string();
        let temperature = temperature.parse::<f32>().unwrap();
        stats.entry(city).or_insert(Record::default()).add(temperature);
    }

    // Collect and sort
    let stats: BTreeMap<String, Record> = stats.into_iter().collect();

    // Output results
    print!("{{");
    let mut stats = stats.into_iter().peekable();
    while let Some((city, r)) = stats.next() {
        print!("{city}={:.1}/{:.1}/{:.1}", r.min, r.avg(), r.max);
        if stats.peek().is_some() {
            print!(", ");
        }
    }
    println!("}}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record() {
        let mut record = Record::default();
        record.add(1.0);
        record.add(2.0);
        assert_eq!(record.min, 1.0);
        assert_eq!(record.max, 2.0);
        assert_eq!(record.sum, 3.0);
        assert_eq!(record.count, 2);
        assert_eq!(record.avg(), 1.5);
    }
}
