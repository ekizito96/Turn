//! Lexer for Turn source. Produces a stream of tokens per spec/02-grammar.md.

use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Spawn,
    Send,
    Receive,
    Vec,
    Turn,
    Let,
    Confidence, // NEW
    Use,
    Context,
    Try,
    Catch,
    Throw,
    Append,
    Remember,
    Recall,
    Call,
    Impl,
    Type, // 'type' keyword for aliases
    Return,
    Struct,
    If,
    Else,
    While,
    And,
    Or,
    True,
    False,
    Null,

    // Types
    TypeNum,
    TypeStr,
    TypeBool,
    TypeList,
    TypeMap,
    TypeAny,
    TypeVoid,
    TypePid,
    TypeVec,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Similarity, // ~>
    Eq, // = (assignment)
    EqEq,
    Ne,
    Less,
    Greater,
    Arrow, // ->

    // Literals
    Num(f64),
    Str(String),

    // Identifier
    Id(String),

    // Punctuation
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Comma,
    Colon,
    Semicolon,
    Dot,

    // End of input
    Eof,
}

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

const KEYWORDS: &[(&str, Token)] = &[
    ("spawn", Token::Spawn),
    ("send", Token::Send),
    ("receive", Token::Receive),
    ("vec", Token::Vec),
    ("turn", Token::Turn),
    ("let", Token::Let),
    ("confidence", Token::Confidence), // NEW
    ("use", Token::Use),
    ("try", Token::Try),
    ("catch", Token::Catch),
    ("throw", Token::Throw),
    ("context", Token::Context),
    ("append", Token::Append),
    ("remember", Token::Remember),
    ("recall", Token::Recall),
    ("call", Token::Call),
    ("return", Token::Return),
    ("struct", Token::Struct),
    ("impl", Token::Impl),
    ("type", Token::Type),
    ("if", Token::If),
    ("else", Token::Else),
    ("while", Token::While),
    ("and", Token::And),
    ("or", Token::Or),
    ("true", Token::True),
    ("false", Token::False),
    ("null", Token::Null),
    ("Num", Token::TypeNum),
    ("Str", Token::TypeStr),
    ("Bool", Token::TypeBool),
    ("List", Token::TypeList),
    ("Map", Token::TypeMap),
    ("Any", Token::TypeAny),
    ("Void", Token::TypeVoid),
    ("Pid", Token::TypePid),
    ("Vec", Token::TypeVec),
];

pub struct Lexer<'a> {
    #[allow(dead_code)]
    source: &'a str,
    chars: Peekable<Chars<'a>>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().peekable(),
            pos: 0,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn start_pos(&self) -> usize {
        self.pos
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.next();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.next() {
            if c == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), LexError> {
        let mut depth = 1;
        while depth > 0 {
            match (self.next(), self.peek()) {
                (Some('*'), Some('/')) => {
                    self.next();
                    depth -= 1;
                }
                (Some('/'), Some('*')) => {
                    self.next();
                    depth += 1;
                }
                (Some(_), _) => {}
                (None, _) => return Err(LexError::UnclosedBlockComment),
            }
        }
        Ok(())
    }

    fn read_string(&mut self) -> Result<Token, LexError> {
        let start = self.start_pos();
        self.next(); // consume opening "
        let mut s = String::new();
        loop {
            match self.next() {
                Some('"') => break,
                Some('\\') => match self.next() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some(c) => return Err(LexError::InvalidEscape(c, start)),
                    None => return Err(LexError::UnclosedString),
                },
                Some(c) => s.push(c),
                None => return Err(LexError::UnclosedString),
            }
        }
        Ok(Token::Str(s))
    }

    fn read_number(&mut self, first: char) -> Token {
        let mut s = String::from(first);
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                s.push(c);
                self.next();
            } else {
                break;
            }
        }
        Token::Num(s.parse().unwrap_or(0.0))
    }

    fn read_identifier(&mut self, first: char) -> Token {
        let mut s = String::from(first);
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                s.push(c);
                self.next();
            } else {
                break;
            }
        }
        for (kw, tok) in KEYWORDS {
            if s == *kw {
                return tok.clone();
            }
        }
        Token::Id(s)
    }

    pub fn next_token(&mut self) -> Result<SpannedToken, LexError> {
        self.skip_whitespace();

        let start = self.start_pos();

        let token = match self.peek() {
            None => Token::Eof,
            Some('~') => {
                self.next();
                if self.peek() == Some('>') {
                    self.next();
                    Token::Similarity
                } else {
                    return Err(LexError::UnexpectedChar('~', start));
                }
            }
            Some('/') => {
                self.next();
                match self.peek() {
                    Some('/') => {
                        self.next();
                        self.skip_line_comment();
                        return self.next_token();
                    }
                    Some('*') => {
                        self.next();
                        self.skip_block_comment()?;
                        return self.next_token();
                    }
                    _ => Token::Slash,
                }
            }
            Some('-') => {
                self.next();
                if self.peek() == Some('>') {
                    self.next();
                    Token::Arrow
                } else {
                    Token::Minus
                }
            }
            Some('"') => self.read_string()?,
            Some(c) if c.is_ascii_digit() => {
                let c = self.next().unwrap();
                self.read_number(c)
            }
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                let c = self.next().unwrap();
                self.read_identifier(c)
            }
            Some('*') => {
                self.next();
                Token::Star
            }
            Some('+') => {
                self.next();
                Token::Plus
            }
            Some('=') => {
                self.next();
                if self.peek() == Some('=') {
                    self.next();
                    Token::EqEq
                } else {
                    Token::Eq
                }
            }
            Some('!') => {
                self.next();
                if self.peek() == Some('=') {
                    self.next();
                    Token::Ne
                } else {
                    return Err(LexError::UnexpectedChar('!', start));
                }
            }
            Some('{') => {
                self.next();
                Token::LBrace
            }
            Some('}') => {
                self.next();
                Token::RBrace
            }
            Some('[') => {
                self.next();
                Token::LBracket
            }
            Some(']') => {
                self.next();
                Token::RBracket
            }
            Some('(') => {
                self.next();
                Token::LParen
            }
            Some(')') => {
                self.next();
                Token::RParen
            }
            Some(',') => {
                self.next();
                Token::Comma
            }
            Some(':') => {
                self.next();
                Token::Colon
            }
            Some(';') => {
                self.next();
                Token::Semicolon
            }
            Some('.') => {
                self.next();
                Token::Dot
            }
            Some('<') => {
                self.next();
                Token::Less
            }
            Some('>') => {
                self.next();
                Token::Greater
            }
            Some(c) => return Err(LexError::UnexpectedChar(c, start)),
        };

        Ok(SpannedToken {
            token,
            span: Span {
                start,
                end: self.pos,
            },
        })
    }

    pub fn tokenize(mut self) -> Result<Vec<SpannedToken>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let t = self.next_token()?;
            let is_eof = matches!(t.token, Token::Eof);
            tokens.push(t);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LexError {
    #[error("unexpected character '{0}' at {1}")]
    UnexpectedChar(char, usize),
    #[error("invalid escape sequence at {1}")]
    InvalidEscape(char, usize),
    #[error("unclosed string")]
    UnclosedString,
    #[error("unclosed block comment")]
    UnclosedBlockComment,
}

impl LexError {
    pub fn offset(&self) -> Option<usize> {
        match self {
            Self::UnexpectedChar(_, pos) | Self::InvalidEscape(_, pos) => Some(*pos),
            Self::UnclosedString | Self::UnclosedBlockComment => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_hello_turn() {
        let src = r#"
turn {
  let name = "Turn";
  remember("user", name);
  context.append("Hello, " + name);
  let out = call("echo", "Hello");
  return out;
}
"#;
        let tokens = Lexer::new(src).tokenize().unwrap();
        let kinds: Vec<_> = tokens.iter().map(|t| &t.token).collect();
        assert!(matches!(kinds[0], Token::Turn));
        assert!(matches!(kinds[1], Token::LBrace));
        assert!(matches!(kinds[2], Token::Let));
        assert!(matches!(kinds[3], Token::Id(_)));
        assert!(matches!(kinds[4], Token::Eq));
        assert!(matches!(kinds[5], Token::Str(_)));
        assert!(matches!(kinds.last(), Some(Token::Eof)));
    }
}
