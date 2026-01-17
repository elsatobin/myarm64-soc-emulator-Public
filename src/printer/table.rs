use colored::Colorize;

enum RowKind {
    Header(String),
    Section(String),
    Content(String),
    Separator,
}

pub struct Table {
    rows: Vec<RowKind>,
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

impl Table {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn header(&mut self, title: &str) {
        self.rows.push(RowKind::Header(title.to_string()));
    }

    pub fn section(&mut self, title: &str) {
        self.rows.push(RowKind::Section(title.to_string()));
    }

    pub fn row(&mut self, content: &str) {
        self.rows.push(RowKind::Content(content.to_string()));
    }

    pub fn separator(&mut self) {
        self.rows.push(RowKind::Separator);
    }

    pub fn print(self) {
        let width = self.compute_width();
        let bar = format!("+{}+", "-".repeat(width));

        for row in &self.rows {
            match row {
                RowKind::Header(title) => {
                    println!("{bar}");
                    println!("| {}{} |", title.bold().cyan(), pad(width - 2 - title.len()));
                    println!("{bar}");
                }
                RowKind::Section(title) => {
                    println!("{bar}");
                    println!("| {}{} |", title.dimmed(), pad(width - 2 - title.len()));
                    println!("{bar}");
                }
                RowKind::Content(content) => {
                    let visible = strip_ansi_len(content);
                    println!("| {}{} |", content, pad(width - 2 - visible));
                }
                RowKind::Separator => {
                    println!("{bar}");
                }
            }
        }

        println!("{bar}");
    }

    fn compute_width(&self) -> usize {
        let mut max_width = 0;

        for row in &self.rows {
            let len = match row {
                RowKind::Header(s) | RowKind::Section(s) => s.len(),
                RowKind::Content(s) => strip_ansi_len(s),
                RowKind::Separator => 0,
            };
            if len > max_width {
                max_width = len;
            }
        }

        max_width + 4
    }
}

fn pad(n: usize) -> String {
    " ".repeat(n)
}

fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            len += 1;
        }
    }
    len
}
