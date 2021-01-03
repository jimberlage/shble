use crate::byte_trie::{ByteTrie, Membership};
use crate::parsers::IndexFilter;
use base64;
use regex::bytes::Regex;

pub struct Options {
    pub separators: Option<ByteTrie>,
    pub regex_filter: Option<Regex>,
    pub index_filters: Option<Vec<IndexFilter>>,
}

impl Options {
    pub fn default() -> Options {
        Options {
            separators: None,
            regex_filter: None,
            index_filters: None,
        }
    }
}

/// Splits string data into parts according to the given separators.
fn split(separators: &ByteTrie, data: Vec<u8>) -> Vec<Vec<u8>> {
    let mut result = vec![];
    let mut current_line = vec![];
    let mut current_separator = vec![];

    for byte in data {
        current_separator.push(byte);
        match separators.membership(current_separator.as_slice()) {
            Membership::NotIncluded => {
                current_line.push(byte);
                if !current_separator.is_empty() {
                    current_separator = vec![];
                }
            }
            Membership::Included if !current_line.is_empty() => {
                result.push(current_line);
                current_line = vec![];
            }
            Membership::Included => (),
            Membership::IncludedAndTerminal if !current_line.is_empty() => {
                result.push(current_line);
                current_line = vec![];
                current_separator = vec![];
            }
            Membership::IncludedAndTerminal => {
                current_separator = vec![];
            }
        };
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }

    result
}

/// Parse the rules for indexes, then keep only entries in the data that match the rules given for indexes.
///
/// This function is a bit atypical in that the rules_str argument is expected to be user input, and has purposefully relaxed parsing logic.
/// It also returns data even in the error case, so that the user still gets some feedback even with invalid input.
/// This is **not** a goal of the rest of the code, in general failing fast is preferred unless there is a strong tie to user input.
fn keep_index_matches(rules: &Vec<IndexFilter>, data: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut result = vec![];

    for i in 0..data.len() {
        if rules.iter().any(|rule| rule.is_match(i)) {
            result.push(data[i].clone());
        }
    }

    result
}

fn keep_regex_matches(regex: &Regex, data: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    data.into_iter()
        .filter(|&field| regex.is_match(field.as_slice()))
        .map(|field| field.clone())
        .collect()
}

fn transform_1d(options: &Options, data: Vec<u8>) -> Vec<Vec<u8>> {
    let mut result = match options.separators {
        None => vec![data],
        Some(separators) => split(&separators, data),
    };

    if let Some(index_filters) = options.index_filters {
        result = keep_index_matches(&index_filters, result);
    }

    if let Some(regex_filter) = options.regex_filter {
        result = keep_regex_matches(&regex_filter, result);
    }

    result
}

pub fn transform_2d(
    line_options: &Options,
    row_options: &Options,
    data: Vec<u8>,
) -> Vec<Vec<Vec<u8>>> {
    transform_1d(line_options, data)
        .into_iter()
        .map(|line| transform_1d(row_options, line))
        .collect()
}

pub fn encode_2d(data: Vec<Vec<Vec<u8>>>) -> Vec<Vec<String>> {
    data.into_iter()
        .map(|line| line.into_iter().map(|row| base64::encode(row)).collect())
        .collect()
}

#[cfg(test)]
mod test {
    use crate::byte_trie::ByteTrie;
    use regex::bytes::Regex;

    fn bytes_vec(data: Vec<&str>) -> Vec<Vec<u8>> {
        data.into_iter().map(|s| s.bytes().collect()).collect()
    }

    #[test]
    fn split() {
        // Special characters are parsed correctly.
        let mut separators = ByteTrie::new();
        separators.insert(&[b'\n']);
        let expected: Vec<Vec<u8>> = bytes_vec(vec!["hi\tthere\tthis", "could\tbe\tcsv"]);
        let actual = super::split(
            &separators,
            "hi\tthere\tthis\ncould\tbe\tcsv".bytes().collect(),
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn keep_index_matches() {
        // The rule "1, 5.." keeps indexes 1, 5, 6, 7, 8.
        let data: Vec<Vec<u8>> = bytes_vec(vec![
            "The", "quick", "brown", "fox", "jumped", "over", "the", "lazy", "dog",
        ]);
        let expected: Vec<Vec<u8>> = bytes_vec(vec!["quick", "over", "the", "lazy", "dog"]);
        let actual = super::keep_index_matches(
            &vec![
                super::IndexFilter::Exact(1usize),
                super::IndexFilter::LowerBounded(5usize),
            ],
            data,
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn keep_regex_matches() {
        // Special characters are parsed correctly.
        let regex = Regex::new("3[0-9]{3}").unwrap();
        let expected = bytes_vec(vec![
            "SystemUIS\t343\tjimberlage\t5u\tIPv4\t0t0\tUDP\t*:3100",
            "rapportd\t379\tjimberlage\t4u\tIPv4\t0t0\tTCP\t*:3001 (LISTEN)",
            "rapportd\t379\tjimberlage\t5u\tIPv6\t0t0\tTCP\t*:3005 (LISTEN)",
        ]);
        let actual = super::keep_regex_matches(
            &regex,
            bytes_vec(vec![
                "COMMAND\tPID\tUSER\tFD\tTYPE\tSIZE/OFF\tNODE\tNAME",
                "loginwind\t168\tjimberlage\t7u\tIPv4\t0t0\tUDP\t*:5678",
                "SystemUIS\t343\tjimberlage\t5u\tIPv4\t0t0\tUDP\t*:3100",
                "SystemUIS\t343\tjimberlage\t8u\tIPv4\t0t0\tUDP\t*:9004",
                "rapportd\t379\tjimberlage\t4u\tIPv4\t0t0\tTCP\t*:3001 (LISTEN)",
                "rapportd\t379\tjimberlage\t5u\tIPv6\t0t0\tTCP\t*:3005 (LISTEN)",
            ]),
        );
        assert_eq!(actual, expected);
    }
}