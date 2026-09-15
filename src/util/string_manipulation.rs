use regex::Regex;

pub fn contains_only_numbers(s: &str) -> bool {
    s.chars().all(|c| c.is_digit(10))
}

pub fn cleanse_string(input: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9_]").unwrap();
    let cleaned_string: std::borrow::Cow<str> = re.replace_all(input, "");
    cleaned_string.to_string()
}
