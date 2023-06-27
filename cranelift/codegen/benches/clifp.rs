use std::iter::Peekable;
use std::str::Chars;

#[derive(Clone, Debug)]
pub enum Token {
    Open,
    Close,
    Float(f64),
    Integer(u128),
    Ident(String),
}

type Stream<'c> = Peekable<Chars<'c>>;

pub fn lex(source: &str) -> Vec<Token> {
    let mut chars = source.chars().peekable();
    let mut toks = vec![];
    'outer: loop {
        for parser in [lex_open, lex_close, lex_float, lex_int, lex_ident] {
            if let Some(tok) = parser(&mut chars) {
                toks.push(tok);
                continue 'outer;
            }
        }

        match chars.next() {
            Some(' ') | Some('\n') | Some('\t') => (),
            None => break 'outer,
            Some(c) => panic!("unexpected character `{}`", c),
        }
    }
    toks
}

fn lex_open(chars: &mut Stream) -> Option<Token> {
    if *chars.peek()? == '(' {
        _ = chars.next().unwrap();
        Some(Token::Open)
    } else {
        None
    }
}

fn lex_close(chars: &mut Stream) -> Option<Token> {
    if *chars.peek()? == ')' {
        _ = chars.next().unwrap();
        Some(Token::Close)
    } else {
        None
    }
}

fn lex_float(in_chars: &mut Stream) -> Option<Token> {
    let mut chars = in_chars.clone();
    if chars.peek()?.is_numeric() {
        let mut str = String::new();
        loop {
            str.push(chars.next().unwrap());
            match chars.peek().map(|r| *r) {
                Some(c) if c.is_numeric() => continue,
                Some(c) if c == '_' => continue,
                Some(c) if c == '.' => break,
                _ => return None,
            }
        }
        loop {
            str.push(chars.next().unwrap());
            match chars.peek() {
                Some(c) if c.is_numeric() => continue,
                _ => break,
            }
        }
        std::mem::replace(in_chars, chars);
        Some(Token::Float(str.parse().unwrap()))
    } else {
        None
    }
}

fn lex_int(chars: &mut Stream) -> Option<Token> {
    if chars.peek()?.is_numeric() {
        let mut str = String::new();
        loop {
            str.push(chars.next().unwrap());
            match chars.peek().map(|r| *r) {
                Some(c) if c.is_numeric() => continue,
                Some(c) if c == '_' => continue,
                _ => break,
            }
        }
        Some(Token::Integer(str.parse().unwrap()))
    } else {
        None
    }
}

fn lex_ident(chars: &mut Stream) -> Option<Token> {
    if chars.peek()?.is_ascii_alphanumeric() {
        let mut str = String::new();
        loop {
            str.push(chars.next().unwrap());
            if !chars
                .peek()
                .map(|r| *r)
                .unwrap_or_default()
                .is_ascii_alphanumeric()
            {
                break;
            }
        }
        Some(Token::Ident(str))
    } else {
        None
    }
}

pub struct Module {
    pub functions: Vec<Function>,
}

pub struct Function {
    pub name: String,
    pub arguments: Vec<(String, String)>,
    pub returns: Vec<String>,
}
