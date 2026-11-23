//! Minimal zero-dependency JSON parser (objects, arrays, strings, numbers, bools, null).

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(m) => m.get(key),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser {
        b: text.as_bytes(),
        i: 0,
    };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.i != p.b.len() {
        return Err(format!("json: trailing data at byte {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if matches!(c, b' ' | b'\n' | b'\r' | b'\t') {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    fn lit(&mut self, s: &str) -> Result<(), String> {
        if self.b[self.i..].starts_with(s.as_bytes()) {
            self.i += s.len();
            Ok(())
        } else {
            Err(format!("json: expected '{}' at byte {}", s, self.i))
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        match self.peek().ok_or("json: unexpected end")? {
            b'n' => {
                self.lit("null")?;
                Ok(Value::Null)
            }
            b't' => {
                self.lit("true")?;
                Ok(Value::Bool(true))
            }
            b'f' => {
                self.lit("false")?;
                Ok(Value::Bool(false))
            }
            b'"' => Ok(Value::String(self.string()?)),
            b'[' => self.array(),
            b'{' => self.object(),
            b'-' | b'0'..=b'9' => self.number(),
            c => Err(format!("json: unexpected byte 0x{:02x}", c)),
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.i += 1; // '['
        let mut out = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Array(out));
        }
        loop {
            self.skip_ws();
            out.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Array(out));
                }
                _ => return Err(format!("json: expected ',' or ']' at byte {}", self.i)),
            }
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.i += 1; // '{'
        let mut out = BTreeMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Object(out));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(format!("json: expected key at byte {}", self.i));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(format!("json: expected ':' at byte {}", self.i));
            }
            self.i += 1;
            self.skip_ws();
            let val = self.value()?;
            out.insert(key, val);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(out));
                }
                _ => return Err(format!("json: expected ',' or '}}' at byte {}", self.i)),
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "json: bad number")?;
        s.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| format!("json: bad number '{}'", s))
    }

    fn hex4(&mut self) -> Result<u16, String> {
        if self.i + 4 > self.b.len() {
            return Err("json: bad unicode escape".into());
        }
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4]).map_err(|_| "json: bad unicode")?;
        let v = u16::from_str_radix(s, 16).map_err(|_| "json: bad unicode".to_string())?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            let c = self.peek().ok_or("json: unterminated string")?;
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.peek().ok_or("json: bad escape")?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..=0xDBFF).contains(&hi) {
                                if self.peek() != Some(b'\\') {
                                    return Err("json: lone surrogate".into());
                                }
                                self.i += 1;
                                if self.peek() != Some(b'u') {
                                    return Err("json: bad surrogate pair".into());
                                }
                                self.i += 1;
                                let lo = self.hex4()?;
                                if !(0xDC00..=0xDFFF).contains(&lo) {
                                    return Err("json: bad surrogate pair".into());
                                }
                                0x10000u32 + ((hi as u32 - 0xD800) << 10) + (lo as u32 - 0xDC00)
                            } else {
                                hi as u32
                            };
                            out.push(char::from_u32(cp).ok_or("json: invalid codepoint")?);
                        }
                        _ => return Err("json: invalid escape".into()),
                    }
                }
                c if c < 0x20 => return Err("json: control character in string".into()),
                c if c < 0x80 => out.push(c as char),
                c => {
                    let len = if c >= 0xF0 {
                        4
                    } else if c >= 0xE0 {
                        3
                    } else if c >= 0xC0 {
                        2
                    } else {
                        return Err("json: invalid utf-8".into());
                    };
                    let start = self.i - 1;
                    if start + len > self.b.len() {
                        return Err("json: invalid utf-8".into());
                    }
                    let slice = &self.b[start..start + len];
                    let s = std::str::from_utf8(slice).map_err(|_| "json: invalid utf-8")?;
                    out.push_str(s);
                    self.i = start + len;
                }
            }
        }
    }
}
