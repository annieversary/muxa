use maud::{Markup, PreEscaped};
use serde::de::{self, Deserialize, Deserializer, Visitor};
use std::{borrow::Cow, path::PathBuf};

/// escapes input, replaces `\n` for `<br/>`, and returns a `PreEscaped` string
pub fn nl2br(s: &str) -> Markup {
    use std::fmt::Write;
    let mut buf = String::new();
    let mut escaper = maud::Escaper::new(&mut buf);
    escaper
        .write_str(s)
        .expect("should be able to write to buffer");

    let out = buf.replace('\n', "<br/>");

    PreEscaped(out)
}

/// wraps the string into an Option, and returns None if it's empty
pub fn empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// returns a list of field names
pub fn struct_fields<'de, T>() -> &'static [&'static str]
where
    T: Deserialize<'de>,
{
    struct StructFieldsDeserializer<'a> {
        fields: &'a mut Option<&'static [&'static str]>,
    }

    impl<'de, 'a> Deserializer<'de> for StructFieldsDeserializer<'a> {
        type Error = serde::de::value::Error;

        fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            Err(de::Error::custom("I'm just here for the fields"))
        }

        fn deserialize_struct<V>(
            self,
            _name: &'static str,
            fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            *self.fields = Some(fields);
            self.deserialize_any(visitor)
        }

        forward_to_deserialize_any! {
            bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
            byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct map enum identifier ignored_any
        }
    }

    let mut fields = None;
    let _ = T::deserialize(StructFieldsDeserializer {
        fields: &mut fields,
    });
    fields.expect("`struct_fields` only works on structs with named fields")
}

/// copies the extension from one file to another
/// `copy_extension("hello.wav", "heyyy.mp3") == "heyyy.wav"`
pub fn copy_extension(from: &str, to: &str) -> PathBuf {
    let from: PathBuf = from.into();
    let to: PathBuf = to.into();
    if let Some(ext) = from.extension() {
        to.with_extension(ext)
    } else {
        to
    }
}

/// truncates to `len` chars, and appends "..." if anything was removed
pub fn truncate_to_length(s: &str, len: usize) -> Cow<'_, str> {
    // char_indices rather than len(), so that a multi byte string isn't reported as
    // truncated when it fits
    match s.char_indices().nth(len) {
        Some((idx, _)) => {
            let mut out = s[..idx].to_string();
            out.push_str("...");
            Cow::Owned(out)
        }
        None => Cow::Borrowed(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert_eq!(empty("".to_string()), None);
        assert_eq!(empty("hey".to_string()), Some("hey".to_string()));
    }

    #[test]
    fn test_truncate_to_length() {
        assert_eq!(truncate_to_length("hello", 10), "hello");
        assert_eq!(truncate_to_length("hello", 5), "hello");
        assert_eq!(truncate_to_length("hello", 3), "hel...");
        // 6 bytes, 3 chars: fits, so it must not gain an ellipsis
        assert_eq!(truncate_to_length("áéí", 3), "áéí");
        assert_eq!(truncate_to_length("áéí", 2), "áé...");
    }

    #[test]
    fn test_copy_extension() {
        fn check(from: &str, to: &str, exp: &str) {
            let p: PathBuf = exp.into();
            assert_eq!(copy_extension(from, to), p);
        }

        check("hello.wav", "heyyy.mp3", "heyyy.wav");
    }
}
