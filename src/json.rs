//! A minimal, zero-dependency JSON parser optimized for binary size.
//! Supports only what is needed for GitHub API responses:
//! - Objects (stored as Vec<(String, JsonValue)>)
//! - Arrays
//! - Strings (with basic escaping)
//! - Numbers (parsed as f64)
//! - Booleans
//! - Null


use std::ops::Index;

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Object(Vec<(String, JsonValue)>),
    Array(Vec<JsonValue>),
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

impl JsonValue {
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(map) => map.iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
        match self {
            JsonValue::Array(arr) => Some(arr),
            _ => None,
        }
    }
}

impl<'a> Index<&'a str> for JsonValue {
    type Output = JsonValue;

    fn index(&self, index: &'a str) -> &Self::Output {
        self.get(index).unwrap_or(&JsonValue::Null)
    }
}

pub fn parse(input: &str) -> Result<JsonValue, String> {
    let mut chars = input.chars().peekable();
    parse_value(&mut chars)
}

fn parse_value(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<JsonValue, String> {
    skip_whitespace(chars);
    
    match chars.peek() {
        Some('{') => parse_object(chars),
        Some('[') => parse_array(chars),
        Some('"') => parse_string(chars).map(JsonValue::String),
        Some('t') | Some('f') => parse_boolean(chars),
        Some('n') => parse_null(chars),
        Some(c) if c.is_digit(10) || *c == '-' => parse_number(chars),
        Some(c) => Err(format!("Unexpected character: {}", c)),
        None => Err("Unexpected end of input".to_string()),
    }
}

fn parse_object(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<JsonValue, String> {
    chars.next(); // consume '{'
    skip_whitespace(chars);
    
    let mut map = Vec::new();
    
    if let Some('}') = chars.peek() {
        chars.next();
        return Ok(JsonValue::Object(map));
    }
    
    loop {
        skip_whitespace(chars);
        let key = parse_string(chars)?;
        
        skip_whitespace(chars);
        if chars.next() != Some(':') {
            return Err("Expected ':' after object key".to_string());
        }
        
        let value = parse_value(chars)?;
        map.push((key, value));
        
        skip_whitespace(chars);
        match chars.next() {
            Some('}') => break,
            Some(',') => continue,
            _ => return Err("Expected '}' or ',' in object".to_string()),
        }
    }
    
    Ok(JsonValue::Object(map))
}

fn parse_array(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<JsonValue, String> {
    chars.next(); // consume '['
    skip_whitespace(chars);
    
    let mut arr = Vec::new();
    
    if let Some(']') = chars.peek() {
        chars.next();
        return Ok(JsonValue::Array(arr));
    }
    
    loop {
        let value = parse_value(chars)?;
        arr.push(value);
        
        skip_whitespace(chars);
        match chars.next() {
            Some(']') => break,
            Some(',') => continue,
            _ => return Err("Expected ']' or ',' in array".to_string()),
        }
    }
    
    Ok(JsonValue::Array(arr))
}

fn parse_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, String> {
    if chars.next() != Some('"') {
        return Err("Expected '\"' for string start".to_string());
    }
    
    let mut s = String::new();
    
    while let Some(c) = chars.next() {
        match c {
            '"' => return Ok(s),
            '\\' => {
                match chars.next() {
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('/') => s.push('/'),
                    Some('b') => s.push('\u{0008}'),
                    Some('f') => s.push('\u{000c}'),
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('t') => s.push('\t'),
                    Some('u') => {
                        let hex: String = chars.take(4).collect();
                        if hex.len() != 4 {
                            return Err("Invalid unicode escape".to_string());
                        }
                        let code = u32::from_str_radix(&hex, 16).map_err(|_| "Invalid hex in unicode escape")?;
                        s.push(std::char::from_u32(code).ok_or("Invalid unicode scalar")?);
                    }
                    Some(c) => return Err(format!("Invalid escape sequence: \\{}", c)),
                    None => return Err("Unterminated string escape".to_string()),
                }
            }
            c => s.push(c),
        }
    }
    
    Err("Unterminated string".to_string())
}

fn parse_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<JsonValue, String> {
    let mut num_str = String::new();
    
    if let Some('-') = chars.peek() {
        num_str.push(chars.next().unwrap());
    }
    
    while let Some(c) = chars.peek() {
        if c.is_digit(10) || *c == '.' || *c == 'e' || *c == 'E' || *c == '+' || *c == '-' {
            num_str.push(chars.next().unwrap());
        } else {
            break;
        }
    }
    
    let n = num_str.parse::<f64>().map_err(|_| format!("Invalid number: {}", num_str))?;
    Ok(JsonValue::Number(n))
}

fn parse_boolean(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<JsonValue, String> {
    if let Some('t') = chars.peek() {
        let val: String = chars.take(4).collect();
        if val == "true" { Ok(JsonValue::Boolean(true)) } else { Err("Invalid boolean".to_string()) }
    } else if let Some('f') = chars.peek() {
        let val: String = chars.take(5).collect();
        if val == "false" { Ok(JsonValue::Boolean(false)) } else { Err("Invalid boolean".to_string()) }
    } else {
        Err("Expected boolean".to_string())
    }
}

fn parse_null(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<JsonValue, String> {
    let val: String = chars.take(4).collect();
    if val == "null" { Ok(JsonValue::Null) } else { Err("Invalid null".to_string()) }
}

fn skip_whitespace(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let json = r#"{"name": "compactrs", "count": 123, "active": true, "ignore": null}"#;
        let parsed = parse(json).unwrap();
        
        assert_eq!(parsed["name"].as_str(), Some("compactrs"));
        assert_eq!(parsed["count"], JsonValue::Number(123.0));
        assert_eq!(parsed["active"], JsonValue::Boolean(true));
        assert_eq!(parsed["ignore"], JsonValue::Null);
    }

    #[test]
    fn test_parse_nested() {
        let json = r#"{"release": {"tag": "v1.0", "assets": [{"name": "bin.exe"}]}}"#;
        let parsed = parse(json).unwrap();
        
        assert_eq!(parsed["release"]["tag"].as_str(), Some("v1.0"));
        let assets = parsed["release"]["assets"].as_array().unwrap();
        assert_eq!(assets[0]["name"].as_str(), Some("bin.exe"));
    }
    
    #[test]
    fn test_escapes() {
        let json = r#"{"msg": "Hello \"World\""}"#;
        let parsed = parse(json).unwrap();
        assert_eq!(parsed["msg"].as_str(), Some("Hello \"World\""));
    }
}
