#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {

    // KEYWORDS
    DEF,
    RETURN,
    IF,
    ELIF,
    ELSE,
    FOR,
    WHILE,
    BREAK,
    CONTINUE,
    IN,
    PASS,
    TRUE,
    FALSE,
    NONE,

    // TYPES
    INT,
    FLOAT,
    BOOL,
    STR,
    LIST,

    // LOGICAL
    AND,
    OR,
    NOT,

    // LITERALS
    INT_LITERAL(i64),
    FLOAT_LITERAL(f64),
    STR_LITERAL(String),
    IDENTIFIER(String),

    // OPERATORS
    PLUS,
    MINUS,
    STAR,
    DOUBLE_STAR,
    MODULO,
    FSLASH,
    DOUBLE_FSLASH,

    // ASSIGNMENT
    EQUAL,
    PLUS_EQUAL,
    MINUS_EQUAL,
    STAR_EQUAL,
    FSLASH_EQUAL,
    DOUBLE_FSLASH_EQUAL,
    MODULO_EQUAL,
    DOUBLE_STAR_EQUAL,

    // COMPARISON
    DOUBLE_EQUAL,
    LESS_THAN,
    LESS_THAN_EQUAL,
    GREATER_THAN,
    GREATER_THAN_EQUAL,

    // DELIMITERS
    COLON,
    COMMA,
    SEMICOLON,
    PERIOD,
    LPAREN,
    RPAREN,
    LBRACKET,
    RBRACKET,
    LBRACE,
    RBRACE,
    ARROW,
    HASHTAG,
    PIPE,

    // SPECIAL
    NEWLINE,
    INDENT,
    DEDENT,
    EOF
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token<'t> {
    pub kind: TokenType,
    pub lexeme: &'t str,
    pub line: usize,
    pub column: usize,
}
