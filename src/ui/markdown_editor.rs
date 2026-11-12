//! Inline Markdown source styling.
use super::views::themed_color;
use iced::advanced::text::{Highlighter, highlighter::Format};
use iced::{Font, Theme, font};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Highlight {
    Heading,
    Bold,
    Italic,
    Code,
    Link,
    Marker,
}
pub struct MarkdownHighlighter {
    line: usize,
    fences: Vec<bool>,
}
impl Highlighter for MarkdownHighlighter {
    type Settings = bool;
    type Highlight = Highlight;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Highlight)>;
    fn new(_: &bool) -> Self {
        Self {
            line: 0,
            fences: vec![false],
        }
    }
    fn update(&mut self, _: &bool) {
        self.change_line(0);
    }
    fn change_line(&mut self, line: usize) {
        self.line = line;
        self.fences.truncate(line + 1);
    }
    fn current_line(&self) -> usize {
        self.line
    }
    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let fenced = self.fences.get(self.line).copied().unwrap_or(false);
        let toggle = line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~");
        self.line += 1;
        self.fences.push(if toggle { !fenced } else { fenced });
        let mut spans = Vec::new();
        if fenced || toggle {
            spans.push((0..line.len(), Highlight::Code));
        } else if line.trim_start().starts_with('#') {
            spans.push((0..line.len(), Highlight::Heading));
        } else {
            let mut offset = 0;
            while offset < line.len() {
                let rest = &line[offset..];
                let token = [
                    ("**", Highlight::Bold),
                    ("__", Highlight::Bold),
                    ("*", Highlight::Italic),
                    ("_", Highlight::Italic),
                    ("`", Highlight::Code),
                ];
                if let Some((delimiter, style)) =
                    token.into_iter().find(|(d, _)| rest.starts_with(d))
                    && let Some(end) = rest[delimiter.len()..].find(delimiter)
                {
                    let end = offset + delimiter.len() * 2 + end;
                    spans.push((offset..end, style));
                    offset = end;
                } else if rest.starts_with('[')
                    && let Some(destination) = rest.find("](")
                    && let Some(end) = rest[destination + 2..].find(')')
                {
                    let end = offset + destination + end + 3;
                    spans.push((offset..end, Highlight::Link));
                    offset = end;
                } else {
                    offset += rest.chars().next().unwrap().len_utf8();
                }
            }
            if spans.is_empty()
                && ["- ", "* ", "> "]
                    .iter()
                    .any(|m| line.trim_start().starts_with(m))
            {
                spans.push((0..line.len(), Highlight::Marker));
            }
        }
        spans.into_iter()
    }
}
pub fn format(style: &Highlight, theme: &Theme) -> Format<Font> {
    let font = match style {
        Highlight::Heading | Highlight::Bold => Font {
            weight: font::Weight::Bold,
            ..Font::DEFAULT
        },
        Highlight::Italic => Font {
            style: font::Style::Italic,
            ..Font::DEFAULT
        },
        Highlight::Code => Font::MONOSPACE,
        _ => Font::DEFAULT,
    };
    Format {
        color: Some(match style {
            Highlight::Code => themed_color(theme, [100, 220, 180], [20, 115, 85]),
            Highlight::Italic => themed_color(theme, [244, 181, 115], [150, 75, 10]),
            Highlight::Bold => themed_color(theme, [242, 150, 205], [155, 35, 110]),
            Highlight::Link => themed_color(theme, [110, 200, 250], [15, 105, 160]),
            Highlight::Marker => theme.palette().success,
            Highlight::Heading => theme.palette().primary,
        }),
        font: Some(font),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_inline_styles_and_fences_survive_reediting() {
        let mut h = MarkdownHighlighter::new(&false);
        let line = "Árvíz **bold** and *italic* `code`";
        let spans: Vec<_> = h.highlight_line(line).collect();
        assert_eq!(
            spans.iter().map(|(_, s)| *s).collect::<Vec<_>>(),
            [Highlight::Bold, Highlight::Italic, Highlight::Code]
        );
        for (range, _) in spans {
            assert!(line.get(range).is_some());
        }
        h.highlight_line("```rust").for_each(drop);
        assert_eq!(
            h.highlight_line("let value = 1;").next().unwrap().1,
            Highlight::Code
        );
        h.change_line(1);
        h.highlight_line("normal text").for_each(drop);
        assert!(h.highlight_line("plain text").next().is_none());
    }
    #[test]
    fn only_complete_markdown_links_receive_link_highlighting() {
        let mut highlighter = MarkdownHighlighter::new(&false);
        for text in ["[plain text)", "[label](unfinished", "[label] plain)"] {
            assert!(
                highlighter
                    .highlight_line(text)
                    .all(|(_, style)| style != Highlight::Link),
                "invalid link: {text}"
            );
        }
        let line = "Before [árvíz](https://example.com) after";
        let spans: Vec<_> = highlighter.highlight_line(line).collect();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].1, Highlight::Link);
        assert_eq!(&line[spans[0].0.clone()], "[árvíz](https://example.com)");
    }

    #[test]
    fn headings_markers_and_fence_edits_restore_the_right_line_state() {
        let mut highlighter = MarkdownHighlighter::new(&false);
        for (line, style) in [
            ("# Heading", Highlight::Heading),
            ("- Item", Highlight::Marker),
            ("> Quoted", Highlight::Marker),
            ("~~~rust", Highlight::Code),
            ("**literal**", Highlight::Code),
            ("~~~", Highlight::Code),
        ] {
            assert_eq!(
                highlighter.highlight_line(line).collect::<Vec<_>>(),
                vec![(0..line.len(), style)]
            );
        }
        assert_eq!(highlighter.current_line(), 6);
        assert!(highlighter.highlight_line("plain").next().is_none());
        highlighter.update(&true);
        assert_eq!(highlighter.current_line(), 0);
        assert_eq!(
            highlighter
                .highlight_line("__bold__ _italic_")
                .map(|(_, style)| style)
                .collect::<Vec<_>>(),
            vec![Highlight::Bold, Highlight::Italic]
        );
        for theme in [Theme::Dark, Theme::Light] {
            for style in [
                Highlight::Heading,
                Highlight::Bold,
                Highlight::Italic,
                Highlight::Code,
                Highlight::Link,
                Highlight::Marker,
            ] {
                let formatted = format(&style, &theme);
                assert!(
                    formatted
                        .color
                        .is_some_and(|color| color.a == 1.0 && color != theme.palette().background)
                );
                let font = formatted.font.unwrap();
                match style {
                    Highlight::Heading | Highlight::Bold => {
                        assert_eq!(font.weight, font::Weight::Bold)
                    }
                    Highlight::Italic => assert_eq!(font.style, font::Style::Italic),
                    Highlight::Code => assert_eq!(font, Font::MONOSPACE),
                    _ => assert_eq!(font, Font::DEFAULT),
                }
            }
        }
    }
}
