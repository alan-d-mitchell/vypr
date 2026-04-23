use super::token::{Token, TokenType};
use error::error::{Span, VyprError};

pub struct Lexer<'l> {
    input: &'l str,
    chars: Vec<char>,
    current: usize,
    start: usize,
    current_byte: usize, 
    start_byte: usize,
    line: usize,
    column: usize,
    pub errors: Vec<VyprError>,
}

impl<'l> Lexer<'l> {

    pub fn new(input: &'l str) -> Self {
        Self {
            input,
            chars: input.chars().collect(),
            current: 0,
            start: 0,
            current_byte: 0,
            start_byte: 0,
            line: 1,
            column: 1,
            errors: Vec::new(),
        }
    }

    fn error(&self, code: &'static str, message: impl Into<String>) -> VyprError {
        VyprError::new(code, message, Span {
            line: self.line,
            column: self.column.saturating_sub(1).max(1),
            length: 1,
        })
    }

    pub fn tokenize(&mut self) -> Vec<Token<'l>> {
        let mut tokens = Vec::new();
        let mut indentations = vec![1];
        let mut line_begin = true;

        while !self.is_at_end() {
            self.skip_whitespace();

            if self.is_at_end() {
                break;
            }

            if line_begin && self.peek() != '\n' {
                let current_indent = self.column;
                let last_indent = indentations.last().copied().unwrap_or(1);

                if current_indent > last_indent {
                    // INDENT
                    indentations.push(current_indent);

                    tokens.push(Token {
                        kind: TokenType::INDENT,
                        lexeme: "",
                        span: Span {
                            line: self.line,
                            column: self.column,
                            length: 0
                        }
                    });
                } else if current_indent < last_indent {
                    // DEDENT
                    while let Some(&top) = indentations.last() {
                        if top == current_indent {
                            break; // found matching level
                        }

                        if top < current_indent {
                            self.errors.push(self.error("L004", "dedent does not match any outer indentation level"));
                            break;
                        }

                        if indentations.len() > 1 {
                            indentations.pop();

                            tokens.push(Token {
                                kind: TokenType::DEDENT,
                                lexeme: "",
                                span: Span {
                                    line: self.line,
                                    column: self.column,
                                    length: 0
                                }
                            });
                        } else {
                            break;
                        }
                    }
                }

                line_begin = false;
            }

            self.start = self.current;
            self.start_byte = self.current_byte;

            match self.scan_token() {
                Ok(Some(kind)) => {
                    if kind == TokenType::NEWLINE {
                        line_begin = true;
                    }

                    let lexeme = &self.input[self.start_offset()..self.current_offset()];

                    tokens.push(Token {
                        kind,
                        lexeme,
                        span: Span {
                            line: self.line,
                            column: self.column,
                            length: lexeme.len().max(1)
                        }
                    });
                }

                Ok(None) => {}
                Err(e) => self.errors.push(e)
            }
        }

        while indentations.len() > 1 {
            indentations.pop();

            tokens.push(Token {
                kind: TokenType::DEDENT,
                lexeme: "",
                span: Span {
                    line: self.line,
                    column: self.column,
                    length: 0,
                }
            });
        }

        tokens.push(Token {
            kind: TokenType::EOF,
            lexeme: "",
            span: Span {
                line: self.line,
                column: self.column,
                length: 0,
            }
        });

        tokens
    }

    fn scan_token(&mut self) -> Result<Option<TokenType>, VyprError> {
        self.skip_whitespace();

        if self.is_at_end() {
            return Ok(None);
        }
        
        let c = self.advance();

        let result = match c {
            '\n' => {
                self.line += 1;
                self.column = 1;

                Ok(Some(TokenType::NEWLINE))
            }
            '"' | '\'' =>  {
                if self.peek() == c && self.peek_next() == c {
                    self.skip_multiline_comment(c)?;

                    Ok(None)
                } else {
                    let kind = self.scan_string(c)?;

                    Ok(Some(kind))
                }
            },
            '(' => Ok(Some(TokenType::LPAREN)),
            ')' => Ok(Some(TokenType::RPAREN)),
            '{' => Ok(Some(TokenType::LBRACE)),
            '}' => Ok(Some(TokenType::RBRACE)),
            '[' => Ok(Some(TokenType::LBRACKET)),
            ']' => Ok(Some(TokenType::RBRACKET)),
            ';' => Ok(Some(TokenType::SEMICOLON)),
            '.' => Ok(Some(TokenType::PERIOD)),
            ':' => Ok(Some(TokenType::COLON)),
            ',' => Ok(Some(TokenType::COMMA)),
            '|' => Ok(Some(TokenType::PIPE)),
            '+' => if self.match_char('=') {
                Ok(Some(TokenType::PLUS_EQUAL))
            } else {
                Ok(Some(TokenType::PLUS))
            },
            '-' => if self.match_char('=') {
                Ok(Some(TokenType::MINUS_EQUAL))
            } else if self.match_char('>') {
                Ok(Some(TokenType::ARROW))
            } else {
                Ok(Some(TokenType::MINUS))
            },
            '*' => if self.match_char('*') {
                if self.peek() == '*' && self.peek_next() == '=' {
                    Ok(Some(TokenType::DOUBLE_STAR_EQUAL))
                } else {
                    Ok(Some(TokenType::DOUBLE_STAR))
                }
            } else if self.match_char('=') {
                Ok(Some(TokenType::STAR_EQUAL))
            } else {
                Ok(Some(TokenType::STAR))
            },
            '/' => if self.match_char('/') {
                if self.peek() == '=' {
                    Ok(Some(TokenType::DOUBLE_FSLASH_EQUAL))
                } else {
                    Ok(Some(TokenType::DOUBLE_FSLASH))
                }
            } else if self.match_char('=') {
                Ok(Some(TokenType::FSLASH_EQUAL))
            } else {
                Ok(Some(TokenType::FSLASH))
            },
            '%' => if self.match_char('=') {
                Ok(Some(TokenType::MODULO_EQUAL))
            } else {
                Ok(Some(TokenType::MODULO))
            },
            '=' => if self.match_char('=') {
                Ok(Some(TokenType::DOUBLE_EQUAL))
            } else {
                Ok(Some(TokenType::EQUAL))
            },
            '<' => if self.match_char('=') {
                Ok(Some(TokenType::LESS_THAN_EQUAL))
            } else {
                Ok(Some(TokenType::LESS_THAN))
            },
            '>' => if self.match_char('=') {
                Ok(Some(TokenType::GREATER_THAN_EQUAL))
            } else {
                Ok(Some(TokenType::GREATER_THAN))
            },

            _ => if c.is_ascii_digit() {
                return self.scan_number(c);
            } else if c.is_ascii_alphabetic() || c == '_' {
                return self.scan_identifier(c);
            } else {
                return Err(self.error("L001", format!("unexpected character '{}'", c)))
            }
        };

        result
    }

    fn scan_string(&mut self, quote: char) -> Result<TokenType, VyprError> {
        let mut value = Vec::new();

        while !self.is_at_end() {
            let c = self.advance();

            match c {
                '\\' => {
                    if self.is_at_end() {
                        return Err(self.error("L002", "unterminated escape sequence in string literal"));
                    }

                    let escaped = self.advance();
                    let escaped_char = match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',

                        _ => {
                            return Err(self.error("L003", format!("unknown escape sequence \\{}", escaped)))
                        }
                    };

                    value.push(escaped_char);
                }

                c if c == quote => {
                    let s: String = value.into_iter().collect();

                    return Ok(TokenType::STR_LITERAL(s));
                }

                '\n' => {
                    return Err(self.error("L002", "unterminated string literal (newline found)"))
                }

                _ => {
                    value.push(c);
                }
            }
        }

        Err(self.error("L002", "unterminated string literal at EOF"))
    }

    fn scan_number(&mut self, first: char) -> Result<Option<TokenType>, VyprError> {
        if first == '0' {
            match self.peek() {
                'x' | 'X' => {
                    self.advance();
                    return self.scan_from_base(16, |c| c.is_ascii_hexdigit())
                }
                'b' | 'B' => {
                    self.advance();
                    return self.scan_from_base(2, |c| c == '0' || c == '1');
                }
                'o' | 'O' => {
                    self.advance();
                    return self.scan_from_base(8, |c| ('0'..='7').contains(&c));
                }
                _ => {}
            }
        }

        let mut text = String::new();
        text.push(first);

        while self.peek().is_ascii_digit() {
            text.push(self.advance());
        }

        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            text.push(self.advance());

            while self.peek().is_ascii_digit() {
                text.push(self.advance());
            }

            let val = text.parse::<f64>()
                .map_err(|_| self.error("L005", "invalid float literal"))?;

            return Ok(Some(TokenType::FLOAT_LITERAL(val)));
        }

        let val = text.parse::<i64>()
            .map_err(|_| self.error("L005", "invalid integer literal"))?; 

        Ok(Some(TokenType::INT_LITERAL(val)))
    }

    fn scan_from_base<F>(&mut self, radix: u32, valid: F) -> Result<Option<TokenType>, VyprError> 
    where F: Fn(char) -> bool
    {
        let mut text = String::new();

        while valid(self.peek()) {
            text.push(self.advance());
        }
        
        if text.is_empty() {
            return Err(self.error("L006", "expected digits after base prefix"))
        }

        let val = i64::from_str_radix(&text, radix)
            .map_err(|_| self.error("L005", "integer literal overflow"))?;

        Ok(Some(TokenType::INT_LITERAL(val)))
    }

    fn scan_identifier(&mut self, _c: char) -> Result<Option<TokenType>, VyprError> {
        while self.peek().is_ascii_alphanumeric() || self.peek() == '_' {
            self.advance();
        }

        let text = &self.input[self.start_offset()..self.current_offset()];

        Ok(Some(self.get_keyword_or_identifier(text)))
    }

    fn get_keyword_or_identifier(&self, text: &str) -> TokenType {
        match text {
            // TYPES
            "int" => TokenType::INT,
            "float" => TokenType::FLOAT,
            "bool" => TokenType::BOOL,
            "str" => TokenType::STR,
            "list" => TokenType::LIST,
            "range" => TokenType::RANGE,

            // KEYWORDS
            "def" => TokenType::DEF,
            "if" => TokenType::IF,
            "elif" => TokenType::ELIF,
            "else" => TokenType::ELSE,
            "for" => TokenType::FOR,
            "while" => TokenType::WHILE,
            "break" => TokenType::BREAK,
            "continue" => TokenType::CONTINUE,
            "in" => TokenType::IN,
            "pass" => TokenType::PASS,
            "return" => TokenType::RETURN,
            "and" => TokenType::AND,
            "or" => TokenType::OR,
            "not" => TokenType::NOT,
            "True" => TokenType::TRUE,
            "False" => TokenType::FALSE,
            "None" => TokenType::NONE,

            _ => TokenType::IDENTIFIER(text.to_string())
        }
    }

    fn advance(&mut self) -> char {
        let c = self.chars[self.current];

        self.current += 1;
        self.column += 1;
        self.current_byte += c.len_utf8();

        c
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.chars[self.current] != expected {
            false
        } else {
            self.advance();

            true
        }
    }

    fn skip_multiline_comment(&mut self, quote: char) -> Result<(), VyprError> {
        self.advance();
        self.advance();

        while !self.is_at_end() {
            let c = self.advance();

            if c == '\n' {
                self.line += 1;
                self.column = 1;
            }

            if c == quote && self.peek() == quote && self.peek_next() == quote {
                self.advance();
                self.advance();

                return Ok(());
            }
        }

        Err(self.error("L007", "unterminated multiline comment"))
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.peek() {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '#' => {
                    while !self.is_at_end() && self.peek() != '\n' {
                        self.advance();
                    }
                }
                _ => return
            }
        }
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.chars[self.current]
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.current + 1]
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.chars.len()
    }

    fn start_offset(&self) -> usize {
        self.start_byte
    }

    fn current_offset(&self) -> usize {
        self.current_byte
    }
}
