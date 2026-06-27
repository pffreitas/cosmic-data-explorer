use syntect::{
    easy::HighlightLines,
    highlighting::{Color, FontStyle, Style, Theme, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

use crate::{DatabaseKind, EngineError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedDocument {
    pub lines: Vec<HighlightedLine>,
}

impl HighlightedDocument {
    pub fn plain_text(&self) -> String {
        self.lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.text.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedLine {
    pub spans: Vec<HighlightedSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedSpan {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub foreground: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone)]
pub struct HighlightService {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl Default for HighlightService {
    fn default() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get("base16-ocean.dark")
            .or_else(|| theme_set.themes.values().next())
            .expect("syntect ships with default themes")
            .clone();

        Self { syntax_set, theme }
    }
}

impl HighlightService {
    pub fn highlight_sql(&self, sql: &str, _dialect: DatabaseKind) -> Result<HighlightedDocument> {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension("sql")
            .or_else(|| self.syntax_set.find_syntax_by_name("SQL"))
            .ok_or_else(|| EngineError::Highlight("SQL syntax is not available".to_string()))?;

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut lines = Vec::new();

        for line in LinesWithEndings::from(sql) {
            let ranges = highlighter
                .highlight_line(line, &self.syntax_set)
                .map_err(|error| EngineError::Highlight(error.to_string()))?;
            lines.push(HighlightedLine {
                spans: ranges_to_spans(ranges),
            });
        }

        if sql.is_empty() {
            lines.push(HighlightedLine { spans: Vec::new() });
        }

        Ok(HighlightedDocument { lines })
    }
}

fn ranges_to_spans(ranges: Vec<(Style, &str)>) -> Vec<HighlightedSpan> {
    let mut start = 0;
    ranges
        .into_iter()
        .map(|(style, text)| {
            let end = start + text.len();
            let span = HighlightedSpan {
                text: text.to_string(),
                start,
                end,
                foreground: color_to_hex(style.foreground),
                bold: style.font_style.contains(FontStyle::BOLD),
                italic: style.font_style.contains(FontStyle::ITALIC),
                underline: style.font_style.contains(FontStyle::UNDERLINE),
            };
            start = end;
            span
        })
        .collect()
}

fn color_to_hex(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}
