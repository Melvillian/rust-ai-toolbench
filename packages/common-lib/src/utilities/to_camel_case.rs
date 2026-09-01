/// Capitalises the first character of `text` and leaves the rest untouched.
///
/// The name is a misnomer inherited from the original library: this does NOT
/// produce camelCase, it only upper-cases the first character. Kept for
/// behavioural parity.
pub fn to_camel_case(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::to_camel_case;

    #[test]
    fn converts_first_letter_to_uppercase() {
        assert_eq!(to_camel_case("hello world!"), "Hello world!");
    }

    #[test]
    fn leaves_the_rest_untouched() {
        assert_eq!(to_camel_case("hELLO"), "HELLO");
        assert_eq!(to_camel_case("Already"), "Already");
        assert_eq!(to_camel_case("123abc"), "123abc");
    }

    #[test]
    fn empty_string_stays_empty() {
        assert_eq!(to_camel_case(""), "");
    }

    #[test]
    fn handles_multibyte_first_character() {
        assert_eq!(to_camel_case("éclair"), "Éclair");
        assert_eq!(to_camel_case("ßtraße"), "SStraße");
    }
}
