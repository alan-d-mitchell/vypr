#[derive(Debug, Clone, Copy, Default)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub length: usize
}

#[derive(Debug, Clone)]
pub struct VyprError {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

impl VyprError {

    pub fn new(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            message: message.into(),
            span,
            help: None
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());

        self
    }

    pub fn report(&self, source: &str, fname: &str) {
        let line = self.span.line.saturating_sub(1);
        let column = self.span.column.saturating_sub(1);
        let span_length = self.span.length.max(1);

        let line_str = source.lines().nth(line).unwrap_or("");

        eprintln!("\nerror[{}]\n{}", self.code, self.message);
        eprintln!("  --> {}:{}:{}", fname, self.span.line, self.span.column);
        eprintln!("   |");
        eprintln!("{:>3} | {}", self.span.line, line_str);
        eprintln!("   | {}{}", " ".repeat(column), "^".repeat(span_length));
        eprintln!("   |");

        if let Some(help) = &self.help {
            eprintln!("      = help: {}", help);
        }
    }
}
