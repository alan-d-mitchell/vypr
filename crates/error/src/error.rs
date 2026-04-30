#[derive(Debug, Clone, Copy, Default, PartialEq)]
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

    pub fn report(&self, source: &str, filename: &str) {
        let line = self.span.line.saturating_sub(1);
        let column = self.span.column.saturating_sub(1);
        let span_length = self.span.length.max(1);

        let line_str = source.lines().nth(line).unwrap_or("");

        let padding: String = line_str
            .chars()
            .take(column)
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();

        let red = "\x1b[31;1m";
        let blue = "\x1b[34;1m";
        let green = "\x1b[32;1m";
        let reset = "\x1b[0m";

        eprintln!("\n{}error[{}]{}\n{}", red, self.code, reset, self.message);
        eprintln!("  {}-->{} {}{}:{}:{}{}", blue, reset, blue, filename, self.span.line, self.span.column, reset);
        eprintln!("   {}|{}", blue, reset);
        eprintln!("{}{:>2} |{} {}", blue, self.span.line, reset, line_str);
        eprintln!("   {}|{} {}{}{}{}", blue, reset, padding, blue, "^".repeat(span_length), reset);
        eprintln!("   {}|{}", blue, reset);

        if let Some(help_msg) = &self.help {
            eprintln!("     {}= help: {}{}", green, help_msg, reset);
        }
    }
}
