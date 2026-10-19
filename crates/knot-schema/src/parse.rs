//! Budget checks occur before building each JSON container, string, or number.
use crate::KnotError;
use serde_json::{Map, Value};

pub const PRODUCT_BYTES: usize = 4 * 1024 * 1024;
pub const STATE_BYTES: usize = 512 * 1024;
pub const EVENT_BYTES: usize = 64 * 1024;
pub const STRING_BYTES: usize = 16 * 1024;

/// Parse UTF-8 JSON without losing raw number precision or duplicate keys.
pub fn parse_json(input: &[u8], maximum: usize) -> Result<Value, KnotError> {
    if input.len() > maximum {
        return Err(KnotError::limit("inputBytes", maximum, input.len()));
    }
    let text = std::str::from_utf8(input).map_err(|_| KnotError::new("INVALID_UTF8", ""))?;
    let mut parser = Parser { text, index: 0 };
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.index != text.len() {
        return Err(parser.bad());
    }
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    index: usize,
}
impl Parser<'_> {
    fn bad(&self) -> KnotError {
        KnotError::new("BAD_JSON", "")
    }
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.index).copied()
    }
    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.index += 1;
        }
    }
    fn take(&mut self, b: u8) -> Result<(), KnotError> {
        if self.peek() != Some(b) {
            return Err(self.bad());
        }
        self.index += 1;
        Ok(())
    }
    fn value(&mut self, depth: usize) -> Result<Value, KnotError> {
        self.whitespace();
        match self.peek() {
            Some(b'{') => {
                if depth >= 64 {
                    return Err(KnotError::limit("jsonDepth", 64, depth + 1));
                }
                self.index += 1;
                self.whitespace();
                let mut map = Map::new();
                if self.peek() == Some(b'}') {
                    self.index += 1;
                    return Ok(Value::Object(map));
                }
                loop {
                    self.whitespace();
                    let key = self.string()?;
                    if map.contains_key(&key) {
                        return Err(self.bad());
                    }
                    self.whitespace();
                    self.take(b':')?;
                    let value = self.value(depth + 1)?;
                    map.insert(key, value);
                    self.whitespace();
                    if self.peek() == Some(b'}') {
                        self.index += 1;
                        break;
                    }
                    self.take(b',')?;
                }
                Ok(Value::Object(map))
            }
            Some(b'[') => {
                if depth >= 64 {
                    return Err(KnotError::limit("jsonDepth", 64, depth + 1));
                }
                self.index += 1;
                self.whitespace();
                let mut items = Vec::new();
                if self.peek() == Some(b']') {
                    self.index += 1;
                    return Ok(Value::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.whitespace();
                    if self.peek() == Some(b']') {
                        self.index += 1;
                        break;
                    }
                    self.take(b',')?;
                }
                Ok(Value::Array(items))
            }
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => {
                self.literal("true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(Value::Bool(false))
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(Value::Null)
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.bad()),
        }
    }
    fn literal(&mut self, s: &str) -> Result<(), KnotError> {
        if !self.text[self.index..].starts_with(s) {
            return Err(self.bad());
        }
        self.index += s.len();
        Ok(())
    }
    fn number(&mut self) -> Result<Value, KnotError> {
        let start = self.index;
        while matches!(
            self.peek(),
            Some(b'0'..=b'9' | b'e' | b'E' | b'+' | b'-' | b'.')
        ) {
            self.index += 1;
            if self.index - start > 128 {
                return Err(KnotError::limit(
                    "numberTokenBytes",
                    128,
                    self.index - start,
                ));
            }
        }
        let token = &self.text[start..self.index];
        if let Some(index) = token.find(['e', 'E']) {
            let exponent = token[index + 1..].parse::<i32>().map_err(|_| self.bad())?;
            if exponent.unsigned_abs() > 308 {
                return Err(KnotError::limit(
                    "exponentMagnitude",
                    308,
                    exponent.unsigned_abs() as usize,
                ));
            }
        }
        serde_json::from_str(token).map_err(|_| self.bad())
    }
    fn hex(&mut self) -> Result<u32, KnotError> {
        let mut n = 0;
        for _ in 0..4 {
            let c = self.peek().ok_or_else(|| self.bad())?;
            let d = char::from(c).to_digit(16).ok_or_else(|| self.bad())?;
            n = n * 16 + d;
            self.index += 1;
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<String, KnotError> {
        self.take(b'"')?;
        let mut result = String::new();
        loop {
            let b = self.peek().ok_or_else(|| self.bad())?;
            if b == b'"' {
                self.index += 1;
                break;
            }
            let c = if b == b'\\' {
                self.index += 1;
                let escaped = self.peek().ok_or_else(|| self.bad())?;
                self.index += 1;
                match escaped {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'/' => '/',
                    b'b' => '\u{8}',
                    b'f' => '\u{c}',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    b'u' => {
                        let first = self.hex()?;
                        let code = if (0xD800..=0xDBFF).contains(&first) {
                            self.take(b'\\')?;
                            self.take(b'u')?;
                            let second = self.hex()?;
                            if !(0xDC00..=0xDFFF).contains(&second) {
                                return Err(self.bad());
                            }
                            0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                        } else {
                            first
                        };
                        char::from_u32(code).ok_or_else(|| self.bad())?
                    }
                    _ => return Err(self.bad()),
                }
            } else {
                if b < 0x20 {
                    return Err(self.bad());
                }
                let c = self.text[self.index..]
                    .chars()
                    .next()
                    .ok_or_else(|| self.bad())?;
                self.index += c.len_utf8();
                c
            };
            if result.len() + c.len_utf8() > STRING_BYTES {
                return Err(KnotError::limit(
                    "stringBytes",
                    STRING_BYTES,
                    result.len() + c.len_utf8(),
                ));
            }
            result.push(c);
        }
        Ok(result)
    }
}
